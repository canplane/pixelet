// pxl-engine/fill.rs

//! A brick's fill: from its tile's samples, its lines and the four palette indices of each
//! cell (`Site`), it writes the brick's words (`clipmap::BRICK_WORDS`). It writes each
//! column's ground top and its cell as laid (`columns`); the ways' spans, worked out once a
//! column from its lines into the brick's scratch (`spans`, `clipmap::SPANS_AT`); from them
//! the brick's levels, level 0 holding each column's own entry (`mip`); and the ways' tree
//! (`way_tree`). The core runs these a part at a time, as its turns have room (pxl-core
//! domain/layer/terrain/clipmap.rs `fill_share`).

use crate::answer;
use crate::area::Shard;
use crate::clipmap::{
    self, column_top, BrickSource, Presence, BOUNDS_AT, BOUND_ENTRY_WORDS, BRICK, BRICK_CELLS,
    COLUMNS_AT, OWN_AT, OWN_CELLS, ROOT_AT, SPANS_ACROSS, SPANS_AT, SPANS_COLUMNS,
    SPANS_COLUMN_WORDS,
};
use crate::{at, ground, way, way_tree, Zoom};

/// Sets word `i` of `s` to `v`. Like `at`, it is total: a store past the slice does nothing,
/// so no store aborts.
pub fn put(s: &mut [u32], i: u32, v: u32) {
    if (i as usize) < s.len() {
        // SAFETY: just bounded by the slice's own length.
        unsafe { *s.get_unchecked_mut(i as usize) = v };
    }
}

/// Column word `i` of the brick at `base` (`clipmap::COLUMN_WORDS`).
pub fn column_of(ground: &[u32], base: u32, i: u32) -> u32 {
    at(ground, base + COLUMNS_AT + i)
}

/// Entry `[x, y]` of level `j` of the brick at `base`, as its two bound words
/// (`clipmap::BOUND_ENTRY_WORDS`).
pub fn mip_of(ground: &[u32], base: u32, j: u32, [x, y]: [u32; 2]) -> [i32; 2] {
    let e = base + BOUNDS_AT + BOUND_ENTRY_WORDS * clipmap::mip_idx(j, [x, y]);
    [at(ground, e) as i32, at(ground, e + 1) as i32]
}

/// Cell `i`'s own palette index as laid, in the brick at `base` (`clipmap::OWN_WORDS`).
pub fn own_of(ground: &[u32], base: u32, i: u32) -> u32 {
    clipmap::own_at(at(ground, base + OWN_AT + i / OWN_CELLS), i)
}

/// The root of the ways' tree of the brick at `base` (`clipmap::ROOT_AT`).
pub fn root_of(ground: &[u32], base: u32) -> u32 {
    at(ground, base + ROOT_AT)
}

/// The brick's own nodes along z that its ways meet, for the brick at `base` with cells at
/// `cell_zoom`, read from the whole bound of its levels (`way_tree::span`): the Z of the
/// first and how many there are, which is what the root of its ways' tree holds.
pub fn root_span(ground: &[u32], base: u32, cell_zoom: Zoom) -> (i32, u32) {
    let ways = ground::extent_of(mip_of(ground, base, clipmap::BRICK_MIPS, [0, 0]))[1];
    way_tree::span(ways, cell_zoom)
}

/// A brick being filled, read on its own, as `mip` folds it.
pub struct LoneBrick<'a> {
    pub ground: &'a [u32],
    pub base: u32,
}

impl Presence for LoneBrick<'_> {
    fn present(&self, _: u32, _: [i32; 2]) -> (u32, bool) {
        (0, true)
    }
}

impl BrickSource for LoneBrick<'_> {
    /// A brick on its own belongs to no clip level.
    fn brick(&self, _: u32, _: u32) -> ([i32; 2], bool) {
        ([0; 2], false)
    }
    fn joined(&self, _: u32, _: u32) -> [u32; clipmap::JOINED_WORDS as usize] {
        [0; clipmap::JOINED_WORDS as usize]
    }
    fn origin(&self, _: u32) -> ([i32; 2], bool) {
        ([0; 2], false)
    }
    /// Its own palette index four times; what `mip` folds reads no palette index.
    fn palette_word(&self, _: u32, _: u32, i: u32) -> u32 {
        ground::word([own_of(self.ground, self.base, i); 4])
    }
    fn column(&self, _: u32, _: u32, i: u32) -> u32 {
        column_of(self.ground, self.base, i)
    }
    fn bound_mip(&self, _: u32, _: u32, j: u32, at: [u32; 2]) -> [i32; 2] {
        mip_of(self.ground, self.base, j, at)
    }
    fn own(&self, _: u32, _: u32, i: u32) -> u32 {
        own_of(self.ground, self.base, i)
    }
    fn root(&self, _: u32, _: u32) -> u32 {
        at(self.ground, self.base + ROOT_AT)
    }
}

/// A brick's spans in its scratch, starting at `at` (`clipmap::SPANS_AT`), as its fill kept
/// them (`spans`). Its levels and its ways' tree read its cells from them.
pub struct Scratch<'a> {
    pub words: &'a [u32],
    pub at: u32,
}

impl Scratch<'_> {
    /// Where column `c`'s spans begin, with `c` in the brick's cells and the ring one cell past
    /// each side.
    fn column(&self, c: [i32; 2]) -> u32 {
        let i = (c[1] + 1) as u32 * SPANS_ACROSS + (c[0] + 1) as u32;
        self.at + SPANS_AT + i * SPANS_COLUMN_WORDS
    }
}

impl way::SpanTable for Scratch<'_> {
    fn header(&self, c: [i32; 2]) -> (i32, u32) {
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

/// The bits of a column's head word that hold its spans' count
/// (`clipmap::SPANS_COLUMN_WORDS`); the bit above them is set where the spans did not fit.
pub const SPANS_CNT: u32 = SPANS_PAST - 1;
pub const SPANS_PAST: u32 = 0x8000;

/// The head word of a column's spans (`clipmap::SPANS_COLUMN_WORDS`).
fn header_word(s: &way::SpanList, fits: bool) -> u32 {
    let face = if s.face == i32::MIN {
        i16::MIN as i32
    } else {
        s.face.clamp(i16::MIN as i32 + 1, i16::MAX as i32)
    };
    s.n | if fits { 0 } else { SPANS_PAST } | (face as u32) << 16
}

/// Where a brick being filled lies, and what it is filled from besides the four palette
/// indices of each cell (`columns`): its cells' zoom, its place in its tile, its first cell as
/// the world names it, what is known of the area there, its tile's padded samples
/// (`ground::ele_idx`) and its lines (`clipmap::LINES_ENTRIES_AT`).
#[derive(Clone, Copy)]
pub struct Site<'a> {
    pub cell_zoom: Zoom,
    pub place: [u32; 2],
    pub first: [i32; 2],
    pub shard: Shard,
    pub ele: &'a [i16],
    pub lines: &'a [u32],
}

impl Site<'_> {
    /// The height of cell `[x, y]`, counted from the brick's first cell, ring cells included
    /// (`ground::height`).
    fn height(&self, [x, y]: [i32; 2]) -> i32 {
        let first = self.place.map(|v| (v * BRICK) as i32);
        ground::height(|i| i32::from(self.ele[i]), [first[0] + x, first[1] + y])
    }
}

/// A brick being filled, as the ways' rule reads it (`clipmap::Brick`): its
/// words in `lane`, and its site.
struct LaneBrick<'a> {
    lane: &'a [u32],
    site: &'a Site<'a>,
}

impl clipmap::Brick for LaneBrick<'_> {
    fn cell_zoom(&self) -> u32 {
        self.site.cell_zoom
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
    fn own(&self, i: u32) -> u32 {
        own_of(self.lane, 0, i)
    }
    /// A column of the brick's own is read as `columns` wrote it (`column_top`), not drawn
    /// again. A ring column, which no column word holds, is drawn from the tile's samples as
    /// `columns` draws a cell.
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

/// Writes a brick's columns and its cells as laid into `lane`, where `four(i)` gives cell
/// `i`'s four palette indices (`ground::palette_idxs`). Each column gets its ground top
/// (`ground::height`); a gone cell, whose own palette index is `UNKNOWN` past the area, gets
/// the Earth's top at sea level. Each cell gets its own palette index (`ground::own` of its
/// four at the brick's zoom), four to a word (`clipmap::OWN_WORDS`).
pub fn columns(lane: &mut [u32], four: impl Fn(u32) -> u32, site: &Site) {
    let mut i = 0;
    while i < BRICK_CELLS {
        let own = ground::own(four(i), site.cell_zoom);
        let [x, y] = clipmap::of_idx(i);
        let top = if own == ground::UNKNOWN {
            crate::SEA_LEVEL
        } else {
            site.height([x as i32, y as i32])
        };
        put(lane, COLUMNS_AT + i, top as u32 & 0xffff);
        let w = OWN_AT + i / OWN_CELLS;
        let k = 8 * (i % OWN_CELLS);
        put(lane, w, (at(lane, w) & !(0xff << k)) | own << k);
        i += 1;
    }
}

/// Writes the spans of column `i` of the brick in `lane`, ring included
/// (`clipmap::SPANS_COLUMNS`), into its scratch (`keep`). Each column's spans are worked out
/// once, before the brick's levels, its ways' tree and its neighbors read them. It returns
/// the most spans the column makes (`keep`).
pub fn spans(lane: &[u32], site: &Site, scratch: &mut [u32], i: u32) -> u32 {
    keep(&LaneBrick { lane, site }, scratch, i)
}

/// Writes the spans of column `i` of brick `sq`, counting row by row from the ring's first
/// column (`clipmap::SPANS_COLUMNS`), from its lines and its face (`way::column`) into the
/// scratch (`clipmap::SPANS_AT`). A corner of the ring lies beside no column of the brick and
/// gets none. It returns the most spans it makes, or `u32::MAX` where they do not fit in
/// `way::SPANS`.
pub fn keep(sq: &impl clipmap::Brick, scratch: &mut [u32], i: u32) -> u32 {
    if i >= SPANS_COLUMNS {
        return 0;
    }
    let c = [(i % SPANS_ACROSS) as i32 - 1, (i / SPANS_ACROSS) as i32 - 1];
    let edge = |v: i32| v == -1 || v == BRICK as i32;
    let (s, fits) = if edge(c[0]) && edge(c[1]) {
        (way::SpanList::NONE, true)
    } else {
        way::column(sq, c)
    };
    let at = SPANS_AT + i * SPANS_COLUMN_WORDS;
    put(scratch, at, header_word(&s, fits));
    let mut j = 0;
    while j < s.n as usize {
        let w = at + 1 + 2 * j as u32;
        let clamp16 = |v: i32| v.clamp(i16::MIN as i32, i16::MAX as i32);
        let run = [clamp16(s.run[j][0]), clamp16(s.run[j][1])];
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

/// Entry `[x, y]` of level `j` of the brick at `base`, whose cells are at `cell_zoom`. At
/// level 0 it is column `[x, y]`'s own entry: its ground's top, and the leaf cells the ways
/// write in it, from the spans its fill kept (`way::lined_hull`; `ground::Extent`). At a
/// higher level it is joined over the level under it, which is written first
/// (`clipmap::join`). A bore's air counts only as far up as the column's top, as the ways
/// write it: the cells of the brick's ways tree under the top lie in the bound (`way_tree`),
/// and a node that the bound splits further holds only air or void there. Above the top a
/// cell is air whatever writes it. A bound reaching up there, past a tunnel's mouth where the
/// ground falls away under the tunnel's clear height, would split every node above it.
pub fn mip(
    ground: &mut [u32],
    base: u32,
    spans: &impl way::SpanTable,
    cell_zoom: Zoom,
    j: u32,
    at: [u32; 2],
) {
    let w = if j == 0 {
        let top = column_top(column_of(ground, base, clipmap::idx_of(at)));
        let lines = way::lined_hull(spans, [at[0] as i32, at[1] as i32], cell_zoom, top);
        ground::extent_word([[top, top], lines])
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
    let e = base + BOUNDS_AT + BOUND_ENTRY_WORDS * clipmap::mip_idx(j, at);
    put(ground, e, w[0] as u32);
    put(ground, e + 1, w[1] as u32);
}

/// Writes every level of the brick in `lane`, whose cells are at `cell_zoom`, from the spans
/// in `scratch`, each level over the one under it (`mip`).
pub fn mips(lane: &mut [u32], scratch: &[u32], cell_zoom: Zoom) {
    let column_spans = Scratch {
        words: scratch,
        at: 0,
    };
    for j in 0..=clipmap::BRICK_MIPS {
        let side = BRICK >> j;
        for k in 0..side * side {
            mip(lane, 0, &column_spans, cell_zoom, j, [k % side, k / side]);
        }
    }
}

/// Fills a brick whole into `lane`, with `scratch` for its spans (`clipmap::SPANS_WORDS`)
/// and `four` giving each cell's four palette indices (`columns`): its columns, its spans and
/// its levels. It returns the most spans a column makes (`u32::MAX` past `way::SPANS`). Its
/// ways' tree is built from the spans afterwards (`tree`).
pub fn brick(lane: &mut [u32], scratch: &mut [u32], four: impl Fn(u32) -> u32, site: &Site) -> u32 {
    columns(lane, four, site);
    let mut most = 0;
    for i in 0..SPANS_COLUMNS {
        most = most.max(spans(lane, site, scratch, i));
    }
    mips(lane, scratch, site.cell_zoom);
    most
}

/// Builds the ways' tree of the brick filled into `lane`, whose spans are in `scratch`
/// (`brick`) and whose cells are at `cell_zoom`, into `pool`, a level at a time
/// (`way_tree`), from the brick's own nodes along z downward. It takes child groups in
/// order from child group `from`, which is the root's; child group 0 means none. It returns
/// the child groups it took, those it still holds once folded, and its root word
/// (`way_tree::root_word`): none taken and 0 where the brick has no ways, and `u32::MAX` for
/// all three where `pool` has too few groups, the tree takes more than
/// `way_tree::BRICK_CHILD_GROUPS`, or its ways span more than `way_tree::ROOT_NODES` of its
/// nodes along z.
pub fn tree(
    lane: &[u32],
    scratch: &[u32],
    cell_zoom: Zoom,
    pool: &mut [u32],
    from: u32,
) -> [u32; 3] {
    use crate::object::CHILDREN;
    use way_tree::{self as t, Column};
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
    let (z0, nodes) = root_span(lane, 0, cell_zoom);
    if nodes == 0 {
        return [0, 0, 0];
    }
    let cap = pool.len() as u32 / t::CHILD_GROUP_WORDS;
    if nodes > t::ROOT_NODES || cap <= from {
        return [u32::MAX; 3];
    }
    let mut p = Column(pool);
    t::root(&mut p, z0, CHILDREN * from);
    // Each level's candidates are the child groups the level over it took, starting from the
    // brick's own zoom.
    let top = cell_zoom - clipmap::BRICK_SHIFT;
    let mut levels = [[0u32; 2]; clipmap::BRICK_SHIFT as usize + 1];
    let (mut b, mut n, mut used) = (CHILDREN * from, CHILDREN, 1);
    let mut zoom = top;
    while zoom < cell_zoom {
        levels[(zoom - top) as usize] = [b, n];
        let flagged = (b..b + n)
            .filter(|&s| t::flag(&p, &brick, 0, cell_zoom, zoom, s))
            .count() as u32;
        if cap < from + used || !t::fits(cap - from - used, used, flagged) {
            return [u32::MAX; 3];
        }
        let next = CHILDREN * (from + used);
        let mut k = 0;
        for s in b..b + n {
            if t::flag(&p, &brick, 0, cell_zoom, zoom, s) {
                t::branch(&mut p, s, next + CHILDREN * k);
                k += 1;
            } else {
                t::empty(&mut p, s);
            }
        }
        (b, n, used, zoom) = (next, CHILDREN * flagged, used + flagged, zoom + 1);
    }
    for s in b..b + n {
        t::leaf(&mut p, &column_spans, cell_zoom, s);
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
    [used, used - freed, t::root_word(from, z0)]
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use crate::answer;
    use crate::object::{self, Cut};
    use crate::way_tree::{self, Column, Read};
    use std::{vec, vec::Vec};

    /// A brick's tree as `tree` builds it, rooted at the brick, reads back through
    /// `way_tree::cursor`, at every cell, exactly what `way::way_cell` writes there and nothing
    /// more. The spans are random, in a patch of columns: slabs, piers and bores over and beside
    /// one another, at the leaves and coarser.
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
            let cell_zoom = crate::LEAF_CELL_ZOOM - round % 3;
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
            // A patch of columns with ways and the rest without, and every column's face.
            let patch = [next(48) as i32, next(48) as i32];
            for i in 0..SPANS_COLUMNS {
                let c = [(i % SPANS_ACROSS) as i32 - 1, (i / SPANS_ACROSS) as i32 - 1];
                let top = tops[i as usize];
                let mut s = way::SpanList::NONE;
                s.face = ground::face_cell(
                    cell_zoom,
                    ground::face_height(cell_zoom, ground::cells(cell_zoom, top as f32)),
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
                put(&mut scratch, at, header_word(&s, true));
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
            mips(&mut lane, &scratch, cell_zoom);
            let column_spans = Scratch {
                words: &scratch,
                at: 0,
            };
            let mut pool = vec![0u32; 16 * (way_tree::BRICK_CHILD_GROUPS as usize + 1)];
            let [used, cnt, root] = tree(&lane, &scratch, cell_zoom, &mut pool, 1);
            assert!(used != u32::MAX && cnt <= used, "round {round}: refused");
            let p = Read(&pool);
            // The tree laid again into a pool shared with another tree, past that tree's groups, in the
            // fewest child groups it holds (`cnt`).
            let (at, mut order) = (3, vec![0; cnt as usize + 1]);
            let mut shared = vec![0u32; 16 * (at + cnt) as usize];
            let laid_root = if root == 0 {
                0
            } else {
                way_tree::lay(&p, root, &mut Column(&mut shared), at, &mut order)
            };
            let q = Read(&shared);
            let s = 1i32 << (crate::LEAF_CELL_ZOOM - cell_zoom);
            for i in 0..BRICK_CELLS {
                let [x, y] = clipmap::of_idx(i);
                for z in (base_top - 60) / s..(base_top + 60) / s {
                    let at = [x as i32, y as i32, z];
                    let slot = way_tree::cursor(&p, root, cell_zoom, cell_zoom, at);
                    let got = way_tree::read(object::val(&p, slot, Cut::NONE, cell_zoom, at));
                    let laid = way_tree::cursor(&q, laid_root, cell_zoom, cell_zoom, at);
                    assert_eq!(
                        got,
                        way_tree::read(object::val(&q, laid, Cut::NONE, cell_zoom, at)),
                        "round {round}: laid as built"
                    );
                    let want = match way::way_cell(&column_spans, [at[0], at[1]], cell_zoom, z) {
                        way::NOTHING => ground::VOID,
                        v => v,
                    };
                    assert_eq!(
                        got, want,
                        "round {round}, cell_zoom {cell_zoom}, column {x}, {y}, z {z}"
                    );
                }
            }
        }
    }
}
