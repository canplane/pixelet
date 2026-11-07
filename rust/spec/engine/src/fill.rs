// pxl-engine/fill.rs

//! A brick's fill, off its tile's samples, its lines and its four palette
//! indices a cell (`Site`), into its words (`clipmap::BRICK_WORDS`): each
//! column's ground top and its cell as laid (`columns`), its ways' spans off
//! its lines, each column's once, into its scratch (`spans`,
//! `clipmap::SPANS_AT`), and off them its levels, level 0 each column's own
//! (`level`), and its ways' tree (`way::tree`). The core runs these a
//! part at a time, as its turns have room (pxl-core
//! domain/layer/terrain/clipmap.rs `fill_share`), and a bake runs them in
//! order over one brick (`brick`), so the spans it sizes a tile's room by are
//! the ones a client keeps.

use crate::answer;
use crate::area::Shard;
use crate::clipmap::{
    self, column_top, Bricks, Presence, BOUNDS_AT, BOUND_ENTRY_WORDS, BRICK, BRICK_CELLS,
    COLUMNS_AT, LAID_AT, LAID_CELLS, ROOT_AT, SPANS_ACROSS, SPANS_AT, SPANS_COLUMNS,
    SPANS_COLUMN_WORDS,
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

/// Column word `i` off the brick at `base` (`clipmap::COLUMN_WORDS`).
pub fn column_of(ground: &[u32], base: u32, i: u32) -> u32 {
    at(ground, base + COLUMNS_AT + i)
}

/// Entry `at` of level `j` off the brick at `base`, its two bound words
/// (`clipmap::BOUND_ENTRY_WORDS`).
pub fn level_of(ground: &[u32], base: u32, j: u32, [x, y]: [u32; 2]) -> [i32; 2] {
    let e = base + BOUNDS_AT + BOUND_ENTRY_WORDS * clipmap::level_idx(j, [x, y]);
    [at(ground, e) as i32, at(ground, e + 1) as i32]
}

/// Cell `i`'s own palette index as laid off the brick at `base`
/// (`clipmap::LAID_WORDS`).
pub fn laid_of(ground: &[u32], base: u32, i: u32) -> u32 {
    clipmap::laid_at(at(ground, base + LAID_AT + i / LAID_CELLS), i)
}

/// The root of the ways' tree off the brick at `base` (`clipmap::ROOT_AT`).
pub fn root_of(ground: &[u32], base: u32) -> u32 {
    at(ground, base + ROOT_AT)
}

/// The brick at `base`'s own nodes along z its ways meet, its cells at
/// `cell`, off its levels' whole bound (`way::tree::span`): what its ways'
/// tree's root holds, the first one's Z and how many.
pub fn root_span(ground: &[u32], base: u32, cell: Zoom) -> (i32, u32) {
    let ways = ground::bounds_of(level_of(ground, base, clipmap::BRICK_LEVELS, [0, 0]))[1];
    way::tree::span(ways, cell)
}

/// A brick being filled, on its own: what `level` folds.
pub struct LoneBrick<'a> {
    pub ground: &'a [u32],
    pub base: u32,
}

impl Presence for LoneBrick<'_> {
    fn present(&self, _: u32, _: [i32; 2]) -> (u32, bool) {
        (0, true)
    }
}

impl Bricks for LoneBrick<'_> {
    /// A brick on its own is no clip level's.
    fn brick(&self, _: u32, _: u32) -> ([i32; 2], bool) {
        ([0; 2], false)
    }
    fn joined(&self, _: u32, _: u32) -> [u32; clipmap::JOINED_WORDS as usize] {
        [0; clipmap::JOINED_WORDS as usize]
    }
    fn origin(&self, _: u32) -> ([i32; 2], bool) {
        ([0; 2], false)
    }
    /// Its own palette index four times: what `level` folds reads no palette index.
    fn palette_word(&self, _: u32, _: u32, i: u32) -> u32 {
        ground::word([laid_of(self.ground, self.base, i); 4])
    }
    fn column(&self, _: u32, _: u32, i: u32) -> u32 {
        column_of(self.ground, self.base, i)
    }
    fn bound_level(&self, _: u32, _: u32, j: u32, at: [u32; 2]) -> [i32; 2] {
        level_of(self.ground, self.base, j, at)
    }
    fn laid(&self, _: u32, _: u32, i: u32) -> u32 {
        laid_of(self.ground, self.base, i)
    }
    fn root(&self, _: u32, _: u32) -> u32 {
        at(self.ground, self.base + ROOT_AT)
    }
}

/// A brick's spans in its scratch from `at` (`clipmap::SPANS_AT`), as its
/// fill kept them (`spans`): what its levels and its ways' tree read its
/// cells off.
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

impl way::ColumnSpans for Scratch<'_> {
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

/// A column's spans' head word's count (`clipmap::SPANS_COLUMN_WORDS`); the
/// bit over it, where they did not fit.
pub const SPANS_CNT: u32 = SPANS_PAST - 1;
pub const SPANS_PAST: u32 = 0x8000;

/// The head word of a column's spans (`clipmap::SPANS_COLUMN_WORDS`).
fn head_word(s: &way::Spans, fits: bool) -> u32 {
    let face = if s.face == i32::MIN {
        i16::MIN as i32
    } else {
        s.face.clamp(i16::MIN as i32 + 1, i16::MAX as i32)
    };
    s.n | if fits { 0 } else { SPANS_PAST } | (face as u32) << 16
}

/// Where a brick being filled lies and what it is filled off, past its four
/// palette indices a cell (`columns`): its cells' zoom, its place in its tile,
/// its first cell as the world names it, what is known of the area at it, its
/// tile's padded samples (`ground::ele_idx`) and its lines
/// (`clipmap::LINES_ENTRIES_AT`).
#[derive(Clone, Copy)]
pub struct Site<'a> {
    pub cell: Zoom,
    pub place: [u32; 2],
    pub first: [i32; 2],
    pub shard: Shard,
    pub ele: &'a [i16],
    pub lines: &'a [u32],
}

impl Site<'_> {
    /// Cell `[x, y]`'s height, counted from the brick's first, its ring's too
    /// (`ground::height`).
    fn height(&self, [x, y]: [i32; 2]) -> i32 {
        let first = self.place.map(|v| (v * BRICK) as i32);
        ground::height(|i| i32::from(self.ele[i]), [first[0] + x, first[1] + y])
    }
}

/// A brick being filled, as the ways' rule reads it (`clipmap::Brick`): its
/// words in `lane`, and its site.
struct Filling<'a> {
    lane: &'a [u32],
    site: &'a Site<'a>,
}

impl clipmap::Brick for Filling<'_> {
    fn cell(&self) -> u32 {
        self.site.cell
    }
    fn place(&self) -> [u32; 2] {
        self.site.place
    }
    fn first(&self) -> [i32; 2] {
        self.site.first
    }
    fn shard(&self) -> Shard {
        self.site.shard
    }
    fn line(&self, k: u32) -> u32 {
        at(self.site.lines, k)
    }
    fn laid(&self, i: u32) -> u32 {
        laid_of(self.lane, 0, i)
    }
    /// A column of the brick's own as `columns` wrote it (`column_top`), read
    /// and not drawn again; the ring about it, which no column word holds,
    /// off its tile's samples as `columns` draws a cell.
    fn height(&self, c: [i32; 2]) -> i32 {
        let side = BRICK as i32;
        if c[0] >= 0 && c[1] >= 0 && c[0] < side && c[1] < side {
            return column_top(column_of(
                self.lane,
                0,
                clipmap::idx_of([c[0] as u32, c[1] as u32]),
            ));
        }
        self.site.height(c)
    }
}

/// A brick's columns and its cells as laid into `lane`, `four` cell `i`'s four
/// palette indices (`ground::palette_indices`): each column's ground top
/// (`ground::height`), a gone cell's (its own palette index `UNKNOWN`, past the
/// area) the Earth's at sea level; and each cell's own palette index
/// (`ground::own` of its four at the brick's zoom), four a word
/// (`clipmap::LAID_WORDS`).
pub fn columns(lane: &mut [u32], four: impl Fn(u32) -> u32, site: &Site) {
    let mut i = 0;
    while i < BRICK_CELLS {
        let own = ground::own(four(i), site.cell);
        let [x, y] = clipmap::of_idx(i);
        let top = if own == ground::UNKNOWN {
            crate::SEA_LEVEL
        } else {
            site.height([x as i32, y as i32])
        };
        put(lane, COLUMNS_AT + i, top as u32 & 0xffff);
        let w = LAID_AT + i / LAID_CELLS;
        let k = 8 * (i % LAID_CELLS);
        put(lane, w, (at(lane, w) & !(0xff << k)) | own << k);
        i += 1;
    }
}

/// The spans of column `i` of the brick in `lane` and its ring
/// (`clipmap::SPANS_COLUMNS`) into its scratch (`keep`): each column's
/// worked out once, before its levels and its ways' tree read them and its
/// neighbors': the most it makes (`keep`).
pub fn spans(lane: &[u32], site: &Site, scratch: &mut [u32], i: u32) -> u32 {
    keep(&Filling { lane, site }, scratch, i)
}

/// Column `i` of brick `sq` and its ring, row by row from the ring's
/// first (`clipmap::SPANS_COLUMNS`), its spans off its lines and its face
/// (`way::column`) into the scratch (`clipmap::SPANS_AT`); a
/// corner of the ring, beside no column of the brick, none. The most
/// spans it makes, `u32::MAX` where they do not fit `way::SPANS`.
pub fn keep(sq: &impl clipmap::Brick, scratch: &mut [u32], i: u32) -> u32 {
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
    let at = SPANS_AT + i * SPANS_COLUMN_WORDS;
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
/// the ways write in it off the spans its fill kept (`way::lined_hull`;
/// `ground::Bounds`); a higher one over the level under it, written before
/// (`clipmap::join`). A bore's air counts as far as the column's top, as the
/// ways write it: the cells of the brick's ways tree under it lie in the
/// bound (`way::tree`), and a node it splits more holds only air or void
/// there. Over the top it is air whatever writes it; a bound reaching up
/// there, past a tunnel's mouth where the ground falls away under its
/// clear height, split every node over it.
pub fn level(
    ground: &mut [u32],
    base: u32,
    spans: &impl way::ColumnSpans,
    cell: Zoom,
    j: u32,
    at: [u32; 2],
) {
    let w = if j == 0 {
        let top = column_top(column_of(ground, base, clipmap::idx_of(at)));
        let lines = way::lined_hull(spans, [at[0] as i32, at[1] as i32], cell, top);
        ground::bounds_word([[top, top], lines])
    } else {
        let under: &[u32] = ground;
        clipmap::join(
            &LoneBrick {
                ground: under,
                base,
            },
            0,
            0,
            j,
            at,
        )
    };
    let e = base + BOUNDS_AT + BOUND_ENTRY_WORDS * clipmap::level_idx(j, at);
    put(ground, e, w[0] as u32);
    put(ground, e + 1, w[1] as u32);
}

/// Every level of the brick in `lane`, off the spans in `scratch`, its cells
/// at `cell`, each over the one under it (`level`).
pub fn levels(lane: &mut [u32], scratch: &[u32], cell: Zoom) {
    let column_spans = Scratch {
        words: scratch,
        at: 0,
    };
    for j in 0..=clipmap::BRICK_LEVELS {
        let side = BRICK >> j;
        for k in 0..side * side {
            level(lane, 0, &column_spans, cell, j, [k % side, k / side]);
        }
    }
}

/// A brick filled whole into `lane`, `scratch` its spans
/// (`clipmap::SPANS_WORDS`): its columns, its spans and its levels, `four` its
/// four palette indices a cell (`columns`). The most spans a column makes
/// (`u32::MAX` past `way::SPANS`); its ways' tree is built off them after
/// (`tree`).
pub fn brick(lane: &mut [u32], scratch: &mut [u32], four: impl Fn(u32) -> u32, site: &Site) -> u32 {
    columns(lane, four, site);
    let mut most = 0;
    for i in 0..SPANS_COLUMNS {
        most = most.max(spans(lane, site, scratch, i));
    }
    levels(lane, scratch, site.cell);
    most
}

/// The brick filled into `lane` and its spans into `scratch` (`brick`),
/// its cells at `cell`, its ways' tree built into `pool`, a level at a time
/// (`way::tree`), from the brick's own nodes
/// along z down, its child groups taken in order from child group `from`, its
/// root's, child group 0 being none's: the child groups it took, those it holds once
/// folded, and its root word (`way::tree::root_word`), none taken and 0
/// where it has no ways; `u32::MAX` all three where `pool` has too few, it
/// takes more than `way::tree::BRICK_CHILD_GROUPS`, or its ways span more
/// than `way::tree::ROOT_NODES` of its nodes along z.
pub fn tree(lane: &[u32], scratch: &[u32], cell: Zoom, pool: &mut [u32], from: u32) -> [u32; 3] {
    use crate::overwrite::CHILDREN;
    use way::tree::{self as t, Column};
    let (brick, column_spans) = (
        LoneBrick {
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
    let room = pool.len() as u32 / t::CHILD_GROUP_WORDS;
    if nodes > t::ROOT_NODES || room <= from {
        return [u32::MAX; 3];
    }
    let mut p = Column(pool);
    t::root(&mut p, z0, CHILDREN * from);
    // Each level's candidates, the child groups the level over it took, from the
    // brick's own zoom.
    let top = cell - clipmap::BRICK_SHIFT;
    let mut levels = [[0u32; 2]; clipmap::BRICK_SHIFT as usize + 1];
    let (mut b, mut n, mut taken) = (CHILDREN * from, CHILDREN, 1);
    let mut zoom = top;
    while zoom < cell {
        levels[(zoom - top) as usize] = [b, n];
        let flagged = (b..b + n)
            .filter(|&s| t::flag(&p, &brick, 0, cell, zoom, s))
            .count() as u32;
        if room < from + taken || !t::fits(room - from - taken, taken, flagged) {
            return [u32::MAX; 3];
        }
        let next = CHILDREN * (from + taken);
        let mut k = 0;
        for s in b..b + n {
            if t::flag(&p, &brick, 0, cell, zoom, s) {
                t::branch(&mut p, s, next + CHILDREN * k);
                k += 1;
            } else {
                t::empty(&mut p, s);
            }
        }
        (b, n, taken, zoom) = (next, CHILDREN * flagged, taken + flagged, zoom + 1);
    }
    for s in b..b + n {
        t::leaf(&mut p, &column_spans, cell, s);
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
    use crate::way::tree::{self, Column, Read};
    use std::{vec, vec::Vec};

    /// A brick's tree as `tree` builds it, its root at the brick, reads
    /// back through `way::tree::cursor` at every cell what `way::way_cell`
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
                vec![0u32; clipmap::BRICK_WORDS as usize],
                vec![0u32; clipmap::SPANS_WORDS as usize],
            );
            let base_top = next(200) as i32 - 100;
            let tops: Vec<i32> = (0..SPANS_COLUMNS)
                .map(|_| base_top + next(9) as i32 - 4)
                .collect();
            for i in 0..BRICK_CELLS {
                let [x, y] = clipmap::of_idx(i);
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
            levels(&mut lane, &scratch, cell);
            let column_spans = Scratch {
                words: &scratch,
                at: 0,
            };
            let mut pool = vec![0u32; 16 * (tree::BRICK_CHILD_GROUPS as usize + 1)];
            let [taken, held, root] = tree(&lane, &scratch, cell, &mut pool, 1);
            assert!(taken != u32::MAX && held <= taken, "round {round}: refused");
            let p = Read(&pool);
            // Laid in a shared pool past another's, its child groups the fewest.
            let (at, mut order) = (3, vec![0; held as usize + 1]);
            let mut shared = vec![0u32; 16 * (at + held) as usize];
            let laid_root = if root == 0 {
                0
            } else {
                tree::lay(&p, root, &mut Column(&mut shared), at, &mut order)
            };
            let q = Read(&shared);
            let s = 1i32 << (crate::LEAF_CELL - cell);
            for i in 0..BRICK_CELLS {
                let [x, y] = clipmap::of_idx(i);
                for z in (base_top - 60) / s..(base_top + 60) / s {
                    let at = [x as i32, y as i32, z];
                    let slot = tree::cursor(&p, root, cell, cell, at);
                    let got = tree::read(overwrite::val(&p, slot, Cut::NONE, cell, at));
                    let laid = tree::cursor(&q, laid_root, cell, cell, at);
                    assert_eq!(
                        got,
                        tree::read(overwrite::val(&q, laid, Cut::NONE, cell, at)),
                        "round {round}: laid as built"
                    );
                    let want = match way::way_cell(&column_spans, [at[0], at[1]], cell, z) {
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
