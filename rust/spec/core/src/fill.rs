// pxl-spec-core/fill.rs

//! gpu-portable: a brick's fill, a thread at a time, off its record in the
//! frame's fresh list (`window::HANDED_WORDS`, its words, looks, samples and
//! lines) into the buffer its words lie in from `base`
//! (`window::BRICK_WORDS`): each column's ground top off its tile's
//! samples and its cell as laid (`heights`), its ways' spans off its lines,
//! each column's once, into its scratch, the frame's (`spans`,
//! `window::SPANS_AT`), and off them its levels, level 0 each column's own
//! (`level`), and its ways' tree (`way::tree`). A GPU runs each pass a
//! thread a column or entry (pxl-shader `height_cs`, `way_cs`,
//! `bound_cs`); a bake runs its columns' passes in order over one brick
//! (`brick`), so the spans it sizes a tile's room by are the ones a GPU
//! keeps.

use crate::answer;
use crate::area;
use crate::area::Shard;
use crate::window::{
    self, column_top, handed_at, Bricks, BRICK, BRICK_CELLS, BRICK_WORDS, COLUMNS_AT, ELE_AT,
    FRESH_BRICK, FRESH_CELL, FRESH_FIRST, FRESH_PLACE, FRESH_SHARD, FRESH_WORDS, LAID_AT,
    LEVELS_AT, LEVEL_ENTRY_WORDS, LINES_AT, LOOKS_AT, ROOT_AT, SPANS_ACROSS, SPANS_AT,
    SPANS_COLUMNS, SPANS_COLUMN_WORDS,
};
use crate::{at, ground, way, Zoom};

/// Word `i` of `s` set to `v`, as `at` reads: totally, none past it, so no
/// store aborts.
pub fn put(s: &mut [u32], i: u32, v: u32) {
    if (i as usize) < s.len() {
        // SAFETY: just bounded by the slice's own length.
        unsafe { *s.get_unchecked_mut(i as usize) = v };
    }
}

/// Column word `i` off the brick at `base` (`window::COLUMN_WORDS`).
pub fn column_of(ground: &[u32], base: u32, i: u32) -> u32 {
    at(ground, base + COLUMNS_AT + i)
}

/// Entry `at` of level `j` off the brick at `base`, its two bound words
/// (`window::LEVEL_ENTRY_WORDS`).
pub fn level_of(ground: &[u32], base: u32, j: u32, [x, y]: [u32; 2]) -> [i32; 2] {
    let e = base + LEVELS_AT + LEVEL_ENTRY_WORDS * window::level_idx(j, [x, y]);
    [at(ground, e) as i32, at(ground, e + 1) as i32]
}

/// Cell `i` as laid off the brick at `base` (`window::LAID_WORDS`).
pub fn laid_of(ground: &[u32], base: u32, i: u32) -> u32 {
    window::laid_at(at(ground, base + LAID_AT + i / 2), i)
}

/// The root of the ways' tree off the brick at `base` (`window::ROOT_AT`).
pub fn root_of(ground: &[u32], base: u32) -> u32 {
    at(ground, base + ROOT_AT)
}

/// The samples handed in the record at `rec` of a fresh list (`ELE_AT`),
/// two a word, low first, each its half's sign carried up:
/// `ground::height`'s.
fn samples(fresh: &[u32], rec: u32) -> impl Fn(usize) -> i32 + '_ {
    move |i: usize| {
        ((at(fresh, rec + ELE_AT + i as u32 / 2) >> (16 * (i as u32 % 2))) << 16) as i32 >> 16
    }
}

/// The handed brick `k` of a fresh list: its record's first word, its
/// brick's first, where it lies in its tile and its cells' zoom.
fn handed(fresh: &[u32], k: u32) -> (u32, u32, [u32; 2], Zoom) {
    let rec = handed_at(k);
    (
        rec,
        at(fresh, rec + FRESH_BRICK) * BRICK_WORDS,
        window::place_of(at(fresh, rec + FRESH_PLACE)),
        at(fresh, rec + FRESH_CELL),
    )
}

/// The cells' zoom of the brick a frame hands `k`-th in `fresh`.
pub fn cell_of(fresh: &[u32], k: u32) -> Zoom {
    at(fresh, handed_at(k) + FRESH_CELL)
}

/// The brick at `base`'s own nodes along z its ways meet, its cells at
/// `cell`, off its levels' whole bound (`way::tree::span`): what its ways'
/// tree's root holds, the first one's Z and how many.
pub fn root_span(ground: &[u32], base: u32, cell: Zoom) -> (i32, u32) {
    let ways = ground::bounds_of(level_of(ground, base, window::LEVELS, [0, 0]))[1];
    way::tree::span(ways, cell)
}

/// A brick being filled, on its own: what `level` folds.
pub struct Fresh<'a> {
    pub ground: &'a [u32],
    pub base: u32,
}

impl Bricks for Fresh<'_> {
    fn present(&self, _: u32, _: [i32; 2]) -> (u32, bool) {
        (0, true)
    }
    /// A brick on its own is no window's.
    fn brick(&self, _: u32, _: u32) -> ([i32; 2], bool) {
        ([0; 2], false)
    }
    fn joined(&self, _: u32, _: u32) -> [u32; window::JOINED_WORDS as usize] {
        [0; window::JOINED_WORDS as usize]
    }
    fn origin(&self, _: u32) -> ([i32; 2], bool) {
        ([0; 2], false)
    }
    /// Its own look four times: what `level` folds reads no look.
    fn look(&self, _: u32, _: u32, i: u32) -> u32 {
        ground::word([laid_of(self.ground, self.base, i) & 0xff; 4])
    }
    fn column(&self, _: u32, _: u32, i: u32) -> u32 {
        column_of(self.ground, self.base, i)
    }
    fn level(&self, _: u32, _: u32, j: u32, at: [u32; 2]) -> [i32; 2] {
        level_of(self.ground, self.base, j, at)
    }
    fn laid(&self, _: u32, _: u32, i: u32) -> u32 {
        laid_of(self.ground, self.base, i)
    }
    fn root(&self, _: u32, _: u32) -> u32 {
        at(self.ground, self.base + ROOT_AT)
    }
}

/// A handed brick's spans in its scratch from `at` (`window::SPANS_AT`),
/// as its fill kept them (`spans`): what its levels and its ways' tree
/// read its cells off.
pub struct Scratch<'a> {
    pub words: &'a [u32],
    pub at: u32,
}

impl Scratch<'_> {
    /// Where column `c`'s spans begin, `c` in the brick's cells, its ring
    /// one past each side.
    fn column(&self, c: [i32; 2]) -> u32 {
        let i = (c[1] + 1) as u32 * SPANS_ACROSS + (c[0] + 1) as u32;
        self.at + SPANS_AT + i * SPANS_COLUMN_WORDS
    }
}

impl way::Kept for Scratch<'_> {
    fn head(&self, c: [i32; 2]) -> (i32, u32) {
        let w = at(self.words, self.column(c));
        let face = w as i32 >> 16;
        (
            if face == i16::MIN as i32 {
                i32::MIN
            } else {
                face
            },
            w & SPANS_CNT,
        )
    }
    fn span(&self, c: [i32; 2], j: u32) -> ([i32; 2], u32) {
        let w = self.column(c) + 1 + 2 * j;
        (
            answer::bound_of(at(self.words, w) as i32),
            at(self.words, w + 1),
        )
    }
}

/// A column's spans' head word's count (`window::SPANS_COLUMN_WORDS`); the
/// bit over it, where they did not fit.
const SPANS_CNT: u32 = SPANS_PAST - 1;
const SPANS_PAST: u32 = 0x8000;

/// The head word of a column's spans (`window::SPANS_COLUMN_WORDS`).
fn head_word(s: &way::Spans, fits: bool) -> u32 {
    let face = if s.face == i32::MIN {
        i16::MIN as i32
    } else {
        s.face.clamp(i16::MIN as i32 + 1, i16::MAX as i32)
    };
    s.n | if fits { 0 } else { SPANS_PAST } | (face as u32) << 16
}

/// Where the brick a frame hands `k`-th has its scratch in the frame's
/// `scratch`, `window::HANDED` bricks' alike, each a `HANDED`th of its
/// length (`way::tree::scratch_words`), which the renderer grows
/// (pxl-render gpu/ray/ground.rs `GroundBuffers::grow_scratch`): taken from
/// the buffer, as the pool's free stack takes its own (pxl-shader `pop`).
/// A one brick's scratch is the first's.
pub fn scratch_at(scratch: &[u32], k: u32) -> u32 {
    k * (scratch.len() as u32 / window::HANDED)
}

/// A brick being filled, as the ways' rule reads it (`window::Brick`):
/// its laid cells in its lane, its lines and samples in its record, and
/// where it is as handed (`window::FRESH_WORDS`).
struct Filling<'a> {
    ground: &'a [u32],
    base: u32,
    fresh: &'a [u32],
    rec: u32,
    cell: u32,
    place: [u32; 2],
}

impl window::Brick for Filling<'_> {
    fn cell(&self) -> u32 {
        self.cell
    }
    fn place(&self) -> [u32; 2] {
        self.place
    }
    fn line(&self, k: u32) -> u32 {
        at(self.fresh, self.rec + LINES_AT + k)
    }
    fn laid(&self, i: u32) -> u32 {
        laid_of(self.ground, self.base, i)
    }
    /// A column of the brick's own as `heights` wrote it (`column_top`),
    /// read and not drawn again; the ring about it, which no column word
    /// holds, off its tile's samples as `heights` draws a cell
    /// (`ground::height`).
    fn height(&self, c: [i32; 2]) -> i32 {
        let side = BRICK as i32;
        if c[0] >= 0 && c[1] >= 0 && c[0] < side && c[1] < side {
            return column_top(column_of(
                self.ground,
                self.base,
                window::idx_of([c[0] as u32, c[1] as u32]),
            ));
        }
        let first = [
            (self.place[0] * BRICK) as i32,
            (self.place[1] * BRICK) as i32,
        ];
        ground::height(
            samples(self.fresh, self.rec),
            [first[0] + c[0], first[1] + c[1]],
        )
    }
}

/// Column `[x, y]` of the brick a frame hands `k`-th in `fresh`: its
/// ground's top (`ground::height`) off its tile's samples, a gone cell's
/// (its own look `UNKNOWN`, past the area) the Earth's at sea level, so it
/// reads as the Earth (`area::laid`); and, a column of an even `x`, its
/// cell and the next as laid, the word they share (`window::LAID_WORDS`),
/// so each word has one writer: its own look off its four at the brick's
/// zoom (`ground::own`), and at a leaf's cells or finer, which of the leaf
/// cells about its own are past the area (`area::gone_ring`), off the
/// brick's shard.
pub fn heights(ground: &mut [u32], fresh: &[u32], k: u32, [x, y]: [u32; 2]) {
    let (rec, base, place, cell) = handed(fresh, k);
    let (first, shard) = (
        [
            at(fresh, rec + FRESH_FIRST) as i32,
            at(fresh, rec + FRESH_FIRST + 1) as i32,
        ],
        Shard::of_word(at(fresh, rec + FRESH_SHARD)),
    );
    let i = window::idx_of([x, y]);
    let own = ground::own(at(fresh, rec + LOOKS_AT + i), cell);
    let top = if own == ground::UNKNOWN {
        crate::SEA_LEVEL
    } else {
        ground::height(
            samples(fresh, rec),
            [(place[0] * BRICK + x) as i32, (place[1] * BRICK + y) as i32],
        )
    };
    put(ground, base + COLUMNS_AT + i, window::column_word(top));
    if x % 2 == 1 {
        return;
    }
    // The leaf cells gone about each: finer than the leaves, the pair lies
    // in one leaf cell, whose ring is both's.
    let gone = |k: u32| {
        let b = [
            crate::wrap_idx(cell, first[0] + (x + k) as i32),
            first[1] + y as i32,
        ];
        let s = cell - crate::LEAF_CELL;
        area::gone_ring(shard, (cell, b), [b[0] >> s, b[1] >> s])
    };
    let gone0 = if cell >= crate::LEAF_CELL { gone(0) } else { 0 };
    let gone1 = if cell == crate::LEAF_CELL {
        gone(1)
    } else {
        gone0
    };
    let next = ground::own(at(fresh, rec + LOOKS_AT + i + 1), cell);
    put(
        ground,
        base + LAID_AT + i / 2,
        window::laid_half(own, gone0) | window::laid_half(next, gone1) << 16,
    );
}

/// Cell `[x, y]`'s four looks of the brick a frame hands `k`-th in
/// `fresh`, as handed (`LOOKS_AT`), into `looks`, a word a cell,
/// `BRICK_CELLS` a brick in the bricks' order: what a ray's hit is
/// drawn by (`ground::Cell::four`), the one read of them past the fill.
pub fn look(looks: &mut [u32], fresh: &[u32], k: u32, [x, y]: [u32; 2]) {
    let rec = handed_at(k);
    let i = window::idx_of([x, y]);
    put(
        looks,
        at(fresh, rec + FRESH_BRICK) * BRICK_CELLS + i,
        at(fresh, rec + LOOKS_AT + i),
    );
}

/// The spans of column `i` of the brick handed `k`-th and its ring
/// (`window::SPANS_COLUMNS`) into its scratch (`keep`): each column's
/// worked out once, before its levels and its ways' tree read them and its
/// neighbors': the most it makes (`keep`).
pub fn spans(ground: &[u32], fresh: &[u32], scratch: &mut [u32], k: u32, i: u32) -> u32 {
    let (rec, base, place, cell) = handed(fresh, k);
    let filling = Filling {
        ground,
        base,
        fresh,
        rec,
        cell,
        place,
    };
    let mine = scratch_at(scratch, k);
    keep(&filling, scratch, mine, i)
}

/// Column `i` of brick `sq` and its ring, row by row from the ring's
/// first (`window::SPANS_COLUMNS`), its spans off its lines and its face
/// (`way::column`) into the scratch from `mine` (`window::SPANS_AT`); a
/// corner of the ring, beside no column of the brick, none. The most
/// spans it makes, `u32::MAX` where they do not fit `way::SPANS`.
pub fn keep(sq: &impl window::Brick, scratch: &mut [u32], mine: u32, i: u32) -> u32 {
    if i >= SPANS_COLUMNS {
        return 0;
    }
    let c = [(i % SPANS_ACROSS) as i32 - 1, (i / SPANS_ACROSS) as i32 - 1];
    let edge = |v: i32| v == -1 || v == BRICK as i32;
    let (s, fits) = if edge(c[0]) && edge(c[1]) {
        (way::Spans::NONE, true)
    } else {
        way::column(sq, c)
    };
    let at = mine + SPANS_AT + i * SPANS_COLUMN_WORDS;
    put(scratch, at, head_word(&s, fits));
    let mut j = 0;
    while j < s.n as usize {
        let w = at + 1 + 2 * j as u32;
        let held = |v: i32| v.clamp(i16::MIN as i32, i16::MAX as i32);
        let run = [held(s.run[j][0]), held(s.run[j][1])];
        put(scratch, w, answer::bound_word(run) as u32);
        put(scratch, w + 1, s.val[j]);
        j += 1;
    }
    if fits {
        s.n
    } else {
        u32::MAX
    }
}

/// Entry `[x, y]` of level `j` of the brick at `base`, its cells at `cell`:
/// level 0 column `[x, y]`'s own — its ground's top, and the leaf cells
/// the ways write in it off the spans its fill kept (`way::bound`;
/// `ground::Bounds`); a higher one over the level under it, written before
/// (`window::join`). A bore's air counts as far as the column's top, as the
/// ways write it: the cells of the brick's ways tree under it lie in the
/// bound (`way::tree`), and a node it splits more holds only air or void
/// there. Over the top it is air whatever writes it; a bound reaching up
/// there, past a tunnel's mouth where the ground falls away under its
/// clear height, split every node over it.
pub fn level(
    ground: &mut [u32],
    base: u32,
    kept: &impl way::Kept,
    cell: Zoom,
    j: u32,
    at: [u32; 2],
) {
    let w = if j == 0 {
        let top = column_top(column_of(ground, base, window::idx_of(at)));
        let lines = way::bound(kept, [at[0] as i32, at[1] as i32], cell, top);
        ground::bounds_word([[top, top], lines])
    } else {
        let under: &[u32] = ground;
        window::join(
            &Fresh {
                ground: under,
                base,
            },
            0,
            0,
            j,
            at,
        )
    };
    let e = base + LEVELS_AT + LEVEL_ENTRY_WORDS * window::level_idx(j, at);
    put(ground, e, w[0] as u32);
    put(ground, e + 1, w[1] as u32);
}

/// A brick filled whole, its passes in a GPU's order (pxl-render
/// gpu/ray/ground.rs `grids`): `fresh` its record (`window::HANDED_WORDS`),
/// its looks, samples and lines as handed, its words laid here; `lane` its
/// words, and `scratch` its scratch (`way::tree::SCRATCH_WORDS`); its cells
/// at `cell`, lying at `place` in its tile, its first cell `first`; its
/// columns, its spans and levels; inside the area, so no wall cuts its ways
/// and the spans are the most a client's fill keeps. The most spans a
/// column makes (`u32::MAX` past `way::SPANS`); its ways' tree is built
/// off them after (`tree`).
pub fn brick(
    fresh: &mut [u32],
    lane: &mut [u32],
    scratch: &mut [u32],
    place: [u32; 2],
    cell: Zoom,
    first: [i32; 2],
) -> u32 {
    fresh[..FRESH_WORDS as usize].copy_from_slice(&window::fresh(
        0,
        window::place_word(place),
        cell,
        first,
        Shard::INSIDE,
    ));
    let fresh: &[u32] = fresh;
    for y in 0..BRICK {
        for x in 0..BRICK {
            heights(lane, fresh, 0, [x, y]);
        }
    }
    let mut most = 0;
    for i in 0..SPANS_COLUMNS {
        most = most.max(spans(lane, fresh, scratch, 0, i));
    }
    let kept = Scratch {
        words: scratch,
        at: 0,
    };
    for j in 0..=window::LEVELS {
        let side = BRICK >> j;
        for k in 0..side * side {
            level(lane, 0, &kept, cell, j, [k % side, k / side]);
        }
    }
    most
}

/// The brick filled into `lane` and its spans into `scratch` (`brick`),
/// its cells at `cell`, its ways' tree built into `pool`, its heap words
/// then its data words, a level at a time as a GPU's workgroup builds it
/// (pxl-shader `way_tree_cs`, `way::tree`), from the brick's own nodes
/// along z down, its families taken in order from family `from`, its
/// root's, family 0 being none's: the families it took, those it holds once
/// folded, and its root word (`way::tree::root_word`), none taken and 0
/// where it has no ways; `u32::MAX` all three where `pool` has too few, it
/// takes more than `way::tree::BRICK_FAMILIES`, or its ways span more
/// than `way::tree::ROOT_NODES` of its nodes along z.
pub fn tree(lane: &[u32], scratch: &[u32], cell: Zoom, pool: &mut [u32], from: u32) -> [u32; 3] {
    use crate::overwrite::FAMILY;
    use way::tree::{self as t, Lane};
    let (brick, kept) = (
        Fresh {
            ground: lane,
            base: 0,
        },
        Scratch {
            words: scratch,
            at: 0,
        },
    );
    let (z0, nodes) = root_span(lane, 0, cell);
    if nodes == 0 {
        return [0, 0, 0];
    }
    let room = pool.len() as u32 / t::FAMILY_WORDS;
    if nodes > t::ROOT_NODES || room <= from {
        return [u32::MAX; 3];
    }
    let mut p = Lane(pool);
    t::root(&mut p, z0, FAMILY * from);
    // Each level's candidates, the families the level over it took, from the
    // brick's own zoom.
    let top = cell - window::BRICK_SHIFT;
    let mut levels = [[0u32; 2]; window::BRICK_SHIFT as usize + 1];
    let (mut b, mut n, mut taken) = (FAMILY * from, FAMILY, 1);
    let mut zoom = top;
    while zoom < cell {
        levels[(zoom - top) as usize] = [b, n];
        let flagged = (b..b + n)
            .filter(|&s| t::flag(&p, &brick, 0, cell, zoom, s))
            .count() as u32;
        if room < from + taken || !t::fits(room - from - taken, taken, flagged) {
            return [u32::MAX; 3];
        }
        let next = FAMILY * (from + taken);
        let mut k = 0;
        for s in b..b + n {
            if t::flag(&p, &brick, 0, cell, zoom, s) {
                t::branch(&mut p, s, next + FAMILY * k);
                k += 1;
            } else {
                t::empty(&mut p, s);
            }
        }
        (b, n, taken, zoom) = (next, FAMILY * flagged, taken + flagged, zoom + 1);
    }
    for s in b..b + n {
        t::leaf(&mut p, &kept, cell, s);
    }
    let mut freed = 0;
    while zoom > top {
        zoom -= 1;
        let [b, n] = levels[(zoom - top) as usize];
        for s in b..b + n {
            if t::under(&p, s) != t::NONE && t::fold(&mut p, zoom, s) != t::NONE {
                freed += 1;
            }
        }
    }
    [taken, taken - freed, t::root_word(from, z0)]
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use crate::answer;
    use crate::overwrite::{self, Cut};
    use crate::way::tree::{self, Read};
    use std::{vec, vec::Vec};

    /// A brick's tree as `tree` builds it, its root at the brick, reads
    /// back through `way::tree::cursor` at every cell what `way::lined`
    /// writes there, and nothing past it: random spans in a patch of
    /// columns, slabs, piers and bores over and beside one another, at the
    /// leaves and coarser.
    #[test]
    fn tree_matches_ways() {
        let mut seed = 0x9e37_79b9_u32;
        let mut next = |m: u32| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed % m
        };
        for round in 0..6 {
            let cell = crate::LEAF_CELL - round % 3;
            let (mut lane, mut scratch) = (
                vec![0u32; BRICK_WORDS as usize],
                vec![0u32; tree::SCRATCH_WORDS as usize],
            );
            let base_top = next(200) as i32 - 100;
            let tops: Vec<i32> = (0..SPANS_COLUMNS)
                .map(|_| base_top + next(9) as i32 - 4)
                .collect();
            for i in 0..BRICK_CELLS {
                let [x, y] = window::of_idx(i);
                let ring = ((y + 1) * SPANS_ACROSS + x + 1) as usize;
                put(&mut lane, COLUMNS_AT + i, tops[ring] as u32 & 0xffff);
            }
            // A patch of columns with ways, the rest none; every column's face.
            let patch = [next(48) as i32, next(48) as i32];
            for i in 0..SPANS_COLUMNS {
                let c = [(i % SPANS_ACROSS) as i32 - 1, (i / SPANS_ACROSS) as i32 - 1];
                let top = tops[i as usize];
                let mut s = way::Spans::NONE;
                s.face = ground::face_cell(
                    cell,
                    ground::face_height(cell, ground::cells(cell, top as f32)),
                );
                let inside =
                    (0..16).contains(&(c[0] - patch[0])) && (0..16).contains(&(c[1] - patch[1]));
                if inside {
                    s.n = next(4);
                    for j in 0..s.n as usize {
                        let lo = top + next(40) as i32 - 30;
                        s.run[j] = [lo, lo + 1 + next(12) as i32];
                        s.val[j] = match next(3) {
                            0 => way::AIR_OF | 3,
                            1 => 7,
                            _ => way::PIER,
                        };
                    }
                }
                let at = SPANS_AT + i * SPANS_COLUMN_WORDS;
                put(&mut scratch, at, head_word(&s, true));
                for j in 0..s.n {
                    let w = at + 1 + 2 * j;
                    put(
                        &mut scratch,
                        w,
                        answer::bound_word(s.run[j as usize]) as u32,
                    );
                    put(&mut scratch, w + 1, s.val[j as usize]);
                }
            }
            let kept = Scratch {
                words: &scratch,
                at: 0,
            };
            for j in 0..=window::LEVELS {
                let side = BRICK >> j;
                for k in 0..side * side {
                    level(&mut lane, 0, &kept, cell, j, [k % side, k / side]);
                }
            }
            let mut pool = vec![0u32; 16 * (tree::BRICK_FAMILIES as usize + 1)];
            let [taken, held, root] = tree(&lane, &scratch, cell, &mut pool, 1);
            assert!(taken != u32::MAX && held <= taken, "round {round}: refused");
            let p = Read(&pool);
            let s = 1i32 << (crate::LEAF_CELL - cell);
            for i in 0..BRICK_CELLS {
                let [x, y] = window::of_idx(i);
                for z in (base_top - 60) / s..(base_top + 60) / s {
                    let at = [x as i32, y as i32, z];
                    let held = tree::cursor(&p, root, cell, cell, at);
                    let got = tree::read(overwrite::val(&p, held, Cut::NONE, cell, at));
                    let want = match way::lined(&kept, [at[0], at[1]], cell, z) {
                        way::NOTHING => ground::VOID,
                        v => v,
                    };
                    assert_eq!(
                        got, want,
                        "round {round}, cell {cell}, column {x}, {y}, z {z}"
                    );
                }
            }
        }
    }
}
