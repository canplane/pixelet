// pxl-portable/window.rs

//! gpu-portable: the ground's cells as they are held, a window a zoom about
//! a centre. A ray observes a zoom's cells only within `RADIUS` of them
//! from the eye (pxl-spec space/layer/demand.rs `band`, the top's by its
//! reach), so a window `WINDOW` cells a side about it holds every cell it
//! can observe there. A window is `ACROSS` squares a side, each square
//! held in the slot its place names modulo `ACROSS`, so the window follows
//! its centre a square at a time and no held cell moves. A square is
//! present once its cells are drawn; a reader of one absent reads the
//! coarser window's (`find`), as a coarser tile stood in before.

use crate::lattice::{GROUND, TILE_CELLS};
use crate::{wrap_index, Zoom, LEAF_CELL};

/// A square's side in cells, as a shift.
pub const SQUARE_SHIFT: u32 = 6;
pub const SQUARE: u32 = 1 << SQUARE_SHIFT;
pub const SQUARE_CELLS: u32 = SQUARE * SQUARE;
/// Squares a window holds a side, and in all.
pub const ACROSS: u32 = 4;
pub const SLOTS: u32 = ACROSS * ACROSS;
/// A window's side in cells.
pub const WINDOW: u32 = ACROSS * SQUARE;
/// The eye's coarsest window's cells' zoom: the top's.
pub const FIRST_CELL: Zoom = GROUND.cell_zoom(GROUND.top());
/// The eye's windows, every zoom the pyramid draws, coarsest first, which
/// the GPU's cast reads: the one at `cell` is `cell − FIRST_CELL`.
pub const EYE_WINDOWS: u32 = LEAF_CELL - FIRST_CELL + 1;
/// Every window: the eye's; the focus's, the leaves' and one over, what
/// walking and editing read; a destination's one, at the zoom its line says
/// its ground, found exactly (pxl-kernel layer/terrain/window.rs).
pub const WINDOWS: u32 = EYE_WINDOWS + 3;
/// Squares every window holds, the eye's first: the most the GPU holds, and
/// so the most of their cells read back at once.
pub const SQUARES: u32 = WINDOWS * SLOTS;
/// A square handed to the GPU's fill, in words: its index, where it lies in
/// its tile (`x | y << 16`, in squares), and its cells' zoom, its window's
/// as the window holds it (pxl-kernel layer/terrain/window.rs `Window::cell`;
/// a destination's moves with its line), never derived from the index.
pub const FRESH_WORDS: u32 = 3;

/// How far from its centre a window holds every cell, in its own cells: a
/// zoom's band ends `SPLIT` of its cells from the eye (`cast::SPLIT`).
pub const RADIUS: u32 = crate::cast::SPLIT as u32;
const _: () = assert!(2 * RADIUS + SQUARE <= WINDOW, "a window holds its radius about any centre in a square");

/// The first square of the window at `cell` about the cell `c`:
/// ⌊(c − RADIUS) / SQUARE⌋ a side, X wrapped round the world. Its squares
/// hold the cells [c − RADIUS, c + RADIUS] wherever in a square `c` lies.
pub fn origin(cell: Zoom, c: [i32; 2]) -> [i32; 2] {
    let first = |v: i32| (v - RADIUS as i32) >> SQUARE_SHIFT;
    [wrap_index(squares_zoom(cell), first(c[0])), first(c[1])]
}

/// The zoom a window's squares are counted at: its cells' over `SQUARE_SHIFT`.
const fn squares_zoom(cell: Zoom) -> Zoom {
    cell - SQUARE_SHIFT
}

/// The slot square `q` is held in.
pub fn slot(q: [i32; 2]) -> u32 {
    let m = (ACROSS - 1) as i32;
    ((q[1] & m) as u32) * ACROSS + (q[0] & m) as u32
}

/// Whether the window at `cell` from `first` holds square `q`, X wrapped.
pub fn covers(cell: Zoom, first: [i32; 2], q: [i32; 2]) -> bool {
    let dx = wrap_index(squares_zoom(cell), q[0] - first[0]);
    let dx = if dx < 0 { dx + (1 << squares_zoom(cell)) } else { dx };
    let dy = q[1] - first[1];
    dx >= 0 && dx < ACROSS as i32 && dy >= 0 && dy < ACROSS as i32
}

/// The square holding cell `g`, and the cell's index in it.
pub fn square_of(g: [i32; 2]) -> ([i32; 2], u32) {
    let m = (SQUARE - 1) as i32;
    ([g[0] >> SQUARE_SHIFT, g[1] >> SQUARE_SHIFT], ((g[1] & m) as u32) * SQUARE + (g[0] & m) as u32)
}

/// Where a square's levels begin among them, in entries: level `j`, from 1,
/// each entry the lowest and highest of the 2^j × 2^j cells under it.
pub const fn level_at(j: u32) -> u32 {
    (SQUARE_CELLS - (SQUARE_CELLS >> (2 * (j - 1)))) / 3
}
pub const LEVELS: u32 = SQUARE_SHIFT;
pub const LEVEL_WORDS: u32 = level_at(LEVELS + 1);

/// A tile's ways are cut into bins `WAYS_ACROSS` a side, each
/// `1 << WAYS_SHIFT` cells: a square lays its lines by them (`LINES_AT`),
/// and its fill writes a bound word a bin off its runs (`bin_word`), which
/// each cell's reach joins (`reach`); an empty one `[i16::MAX, i16::MIN]`,
/// which a join passes over.
pub const WAYS_ACROSS: u32 = 32;
pub const WAYS_SHIFT: u32 = (TILE_CELLS / WAYS_ACROSS).trailing_zeros();
const WAYS_BINS_ACROSS: u32 = SQUARE >> WAYS_SHIFT;
pub const WAYS_BINS: u32 = WAYS_BINS_ACROSS * WAYS_BINS_ACROSS;

/// The most entries a square holds of its tile's ways: one a segment and a
/// bin of the bound (`WAYS_SHIFT`) its footprint reaches. The densest square
/// of the format-10 set, z15 010332102333322 (2026-10-15); a tile past it
/// fails its bake (pxl-spec bake/tile/terrain.rs `ways_fit`).
pub const WAYS_SQUARE_SEGMENTS: u32 = 1068;

/// A square's lines, laid by the CPU as the square is filled (pxl-spec
/// bake/tile/terrain.rs `square_lines`) for the one rule to ask of a block
/// there (`ways::block`): its tile's header (`ways::header`), a word a bin
/// (`WAYS_BINS`), its first entry and its count over it; the entries, two a
/// word, each one of the square's segments, a bin's in their order; then
/// the segments a bin reaches, in their lines' order, `ways::SEGMENT_WORDS`
/// each. None holds more than `WAYS_SQUARE_SEGMENTS` of either.
pub const LINES_ENTRIES_AT: u32 = crate::ways::HEADER_WORDS + WAYS_BINS;
pub const LINES_SEGMENTS_AT: u32 = LINES_ENTRIES_AT + WAYS_SQUARE_SEGMENTS.div_ceil(2);
pub const LINES_WORDS: u32 = LINES_SEGMENTS_AT + WAYS_SQUARE_SEGMENTS * crate::ways::SEGMENT_WORDS as u32;

/// The most runs a square's columns read as (`ways::run`): the most of the
/// format-10 set (2026-10-16), measured with every tile at every zoom (z15's
/// 4312; z14's most 2654); a tile past it fails its bake (pxl-spec
/// bake/tile/terrain.rs `runs_fit`).
pub const RUNS_SQUARE: u32 = 4312;

/// A square's ways as the descent reads them, written at fill (`ways_cs`)
/// off its lines: each cell's first run, a word, cells row by row and one
/// more after the last, so a cell's runs are its first to the next's — each
/// cell's count until the fill sums them; then from `RUNS_RUN_AT` the runs
/// (`ways::run_word`), cells in order and a cell's lowest first. Only cells
/// a way writes in have any.
pub const RUNS_RUN_AT: u32 = SQUARE_CELLS + 1;
pub const RUNS_WORDS: u32 = RUNS_RUN_AT + RUNS_SQUARE;

/// A square's words on a GPU, its lanes in order: its four looks a cell, its
/// heights two a word, its levels, its ways' bound a bin, its tile's padded
/// samples, two a word, its lines, then its ways' runs. Its heights, levels
/// and ways' bound follow one another, read back whole.
pub const LOOK_WORDS: u32 = SQUARE_CELLS;
pub const HEIGHT_WORDS: u32 = SQUARE_CELLS / 2;
pub const ELE_WORDS: u32 = crate::ground::ELE_WORDS;
pub const SQUARE_WORDS: u32 = LOOK_WORDS + HEIGHT_WORDS + LEVEL_WORDS + WAYS_BINS + ELE_WORDS + LINES_WORDS + RUNS_WORDS;
pub const HEIGHTS_AT: u32 = LOOK_WORDS;
pub const LEVELS_AT: u32 = HEIGHTS_AT + HEIGHT_WORDS;
pub const WAYS_AT: u32 = LEVELS_AT + LEVEL_WORDS;
pub const ELE_AT: u32 = WAYS_AT + WAYS_BINS;
pub const LINES_AT: u32 = ELE_AT + ELE_WORDS;
pub const RUNS_AT: u32 = LINES_AT + LINES_WORDS;
/// What comes back of a square, from `HEIGHTS_AT`.
pub const BACK_WORDS: u32 = HEIGHT_WORDS + LEVEL_WORDS + WAYS_BINS;

/// What cell `i` of a square reaches: its height, joined with its bin's
/// ways (`Squares::way`).
pub fn reach(w: &impl Squares, cell: Zoom, slot: u32, i: u32) -> [i32; 2] {
    let h = w.height(cell, slot, i);
    let bin = ((i / SQUARE) >> WAYS_SHIFT) * WAYS_BINS_ACROSS + ((i % SQUARE) >> WAYS_SHIFT);
    let [lo, hi] = crate::ground::bound_of(w.way(cell, slot, bin) as i32);
    [h.min(lo), h.max(hi)]
}

/// The windows as one side holds them: per window, a zoom's cells about
/// its centre, which square each slot holds once present, and its cells.
pub trait Squares {
    /// Where square `q` at `cell` is held present: a slot of the side's own
    /// naming, which the reads below take; false where no window holds it.
    fn present(&self, cell: Zoom, q: [i32; 2]) -> (u32, bool);
    /// The four looks of cell `i` of the square in `slot` (`ground::looks`).
    fn look(&self, cell: Zoom, slot: u32, i: u32) -> u32;
    /// Cell `i`'s height, in leaf cells, as the GPU drew it.
    fn height(&self, cell: Zoom, slot: u32, i: u32) -> i32;
    /// Bin `k`'s ways' bound of the square in `slot`, a bound word
    /// (`bin_word`).
    fn way(&self, cell: Zoom, slot: u32, k: u32) -> u32;
    /// Entry `at` of level `j` of the square in `slot`, as a bound word
    /// (`ground::bound_word`): its cells' reach joined (`join`).
    fn level(&self, cell: Zoom, slot: u32, j: u32, at: [u32; 2]) -> i32;
}

/// One square as its fill reads it, for the ways' one rule
/// (`ways::Columns`): its cells' zoom, where it lies in its tile (in
/// squares), word `k` of its lines (`LINES_AT`), and cell `i`'s four looks
/// and height as its lanes hold them. Its columns are its cells, counted
/// from its first; one outside it has no segment.
pub trait Square {
    fn cell(&self) -> Zoom;
    fn place(&self) -> [u32; 2];
    fn line(&self, k: u32) -> u32;
    fn looks(&self, i: u32) -> u32;
    fn height(&self, i: u32) -> i32;
}

/// Column `c`'s cell index in the square, none outside it.
fn cell_index(c: [i32; 2]) -> u32 {
    if c[0] < 0 || c[1] < 0 || c[0] >= SQUARE as i32 || c[1] >= SQUARE as i32 {
        return SQUARE_CELLS;
    }
    c[1] as u32 * SQUARE + c[0] as u32
}

/// Column `c`'s bin word: its first entry and its count; none outside.
fn bin_of(sq: &impl Square, c: [i32; 2]) -> (u32, u32) {
    if cell_index(c) == SQUARE_CELLS {
        return (0, 0);
    }
    let b = ((c[1] as u32) >> WAYS_SHIFT) * WAYS_BINS_ACROSS + ((c[0] as u32) >> WAYS_SHIFT);
    let w = sq.line(crate::ways::HEADER_WORDS + b);
    (w & 0xffff, w >> 16)
}

/// Entry `e` of the square's lines: the place of a segment among its.
fn entry_of(sq: &impl Square, e: u32) -> u32 {
    (sq.line(LINES_ENTRIES_AT + e / 2) >> (16 * (e % 2))) & 0xffff
}

fn segment_at(sq: &impl Square, at: u32) -> crate::ways::Segment {
    let base = LINES_SEGMENTS_AT + at * crate::ways::SEGMENT_WORDS as u32;
    crate::ways::Segment::of([sq.line(base), sq.line(base + 1), sq.line(base + 2), sq.line(base + 3)])
}

fn header_of(sq: &impl Square) -> (f32, crate::ways::Stations) {
    crate::ways::of_header([sq.line(0), sq.line(1), sq.line(2), sq.line(3), sq.line(4), sq.line(5), sq.line(6), sq.line(7)])
}

/// A square's columns as its lines and its cells give them.
impl<Q: Square> crate::ways::Columns for Q {
    fn cell(&self) -> Zoom {
        Square::cell(self)
    }
    fn per_m(&self) -> f32 {
        header_of(self).0
    }
    fn count(&self, c: [i32; 2]) -> u32 {
        bin_of(self, c).1
    }
    fn segment(&self, c: [i32; 2], k: u32) -> crate::ways::Segment {
        segment_at(self, entry_of(self, bin_of(self, c).0 + k))
    }
    /// In its tile's cells: the square's first, then the cell's middle.
    fn center(&self, c: [i32; 2]) -> [f32; 2] {
        let p = self.place();
        [(p[0] * SQUARE) as f32 + c[0] as f32 + 0.5, (p[1] * SQUARE) as f32 + c[1] as f32 + 0.5]
    }
    fn side(&self, _: [i32; 2]) -> f32 {
        1.0
    }
    fn stations(&self, _: [i32; 2]) -> crate::ways::Stations {
        header_of(self).1
    }
    /// Its own look, as the grid reads a cell before the service area's
    /// wall is laid: the wall is the descent's to lay (`ran`'s reader).
    fn look(&self, c: [i32; 2]) -> u32 {
        let i = cell_index(c);
        if i == SQUARE_CELLS {
            crate::cube::sentinel::void as u32
        } else {
            crate::ground::own(self.looks(i), Square::cell(self))
        }
    }
    fn face(&self, c: [i32; 2]) -> i32 {
        let i = cell_index(c);
        if i == SQUARE_CELLS {
            return 0;
        }
        let cell = Square::cell(self);
        crate::ground::face_height(cell, crate::ground::blocks(cell, self.height(i) as f32))
    }
    /// The entry is the one after the one before it, and its segment
    /// continues the one stored before it.
    fn joins(&self, c: [i32; 2], k: u32) -> bool {
        if k == 0 {
            return false;
        }
        let first = bin_of(self, c).0;
        let e = entry_of(self, first + k);
        e == entry_of(self, first + k - 1) + 1 && segment_at(self, e).continues == 1
    }
}

/// What the ways' runs read of a side's windows beyond their cells: word
/// `k` of the runs lane (`RUNS_AT`) of the square in `slot` at `cell`.
pub trait Ran: Squares {
    fn ran(&self, cell: Zoom, slot: u32, k: u32) -> u32;
}

/// Where a column of blocks at `zoom` has its ways read, found once: the
/// window its ground is read off (`find`'s, its cells past the leaves a
/// leaf's) — its zoom and slot, the square held there — and whether any is;
/// by default none.
#[derive(Clone, Copy, Default)]
pub struct Found {
    pub from: Zoom,
    pub slot: u32,
    pub square: [i32; 2],
    pub here: bool,
}

/// The cell at `from` column `c` of blocks at `zoom` lies in.
fn cell_at(zoom: Zoom, from: Zoom, c: [i32; 2]) -> [i32; 2] {
    let s = zoom - from;
    [wrap_index(from, c[0] >> s), c[1] >> s]
}

/// The window holding cell `g` at `cell`, as `find` finds it to `top`
/// (`Found`), and the cell's index in its square there.
pub fn found_at(w: &impl Squares, cell: Zoom, g: [i32; 2], top: Zoom) -> (Found, u32) {
    let (from, slot, i, here) = find(w, cell, g, top);
    let square = square_of(cell_at(cell, from, g)).0;
    (Found { from, slot, square, here }, i)
}

/// Column `c` of blocks at `zoom`'s window (`Found`), to `top`.
pub fn found(w: &impl Ran, zoom: Zoom, c: [i32; 2], top: Zoom) -> Found {
    let read = if zoom > crate::LEAF_CELL { crate::LEAF_CELL } else { zoom };
    let s = zoom - read;
    found_at(w, read, [c[0] >> s, c[1] >> s], top).0
}

/// What the ways write in block `z` of the column `c` of blocks at `zoom`,
/// as its window holds it run along z (`ways::at`'s): the block it lies in
/// there. A column whose cell lies in `near`'s square is read off it, as
/// the columns about one mostly are; one past it finds its own (`found`).
/// `ways::NOTHING` where no way writes, or no window to `top` holds it.
pub fn ran(w: &impl Ran, near: &Found, zoom: Zoom, c: [i32; 2], z: i32, top: Zoom) -> u32 {
    let (q, _) = square_of(cell_at(zoom, near.from, c));
    // Component-wise: a GPU has no array equality.
    let f = if near.here && q[0] == near.square[0] && q[1] == near.square[1] {
        *near
    } else {
        found(w, zoom, c, top)
    };
    if !f.here {
        return crate::ways::NOTHING;
    }
    let i = square_of(cell_at(zoom, f.from, c)).1;
    let (zr, to) = (z >> (zoom - f.from), w.ran(f.from, f.slot, i + 1));
    let mut r = w.ran(f.from, f.slot, i);
    while r < to {
        let (lo, len, value) = crate::ways::run_of(w.ran(f.from, f.slot, RUNS_RUN_AT + r));
        if zr < lo {
            return crate::ways::NOTHING;
        }
        if zr < lo + len as i32 {
            return value;
        }
        r += 1;
    }
    crate::ways::NOTHING
}

/// The ways' word of bin `b` of a square whose runs are written
/// (`RUNS_AT`), word `k` of its runs lane `run(k)`, its cells at `cell`: the
/// leaf cells its cells' runs span, and of the cells beside them in the
/// square their air, a block either side — the ground `ways::block`
/// recolours beside air, and over and under its own — so no block the rule
/// writes or recolours there lies outside it; empty where it has none.
pub fn bin_word(cell: Zoom, run: impl Fn(u32) -> u32, b: u32) -> i32 {
    let (n, s) = (1 << WAYS_SHIFT, 1 << (crate::LEAF_CELL - cell));
    let first = [(b % WAYS_BINS_ACROSS) * n, (b / WAYS_BINS_ACROSS) * n];
    let (mut lo, mut hi) = (i32::MAX, i32::MIN);
    let mut y = if first[1] > 0 { first[1] - 1 } else { 0 };
    while y <= (first[1] + n).min(SQUARE - 1) {
        let mut x = if first[0] > 0 { first[0] - 1 } else { 0 };
        while x <= (first[0] + n).min(SQUARE - 1) {
            let i = y * SQUARE + x;
            let inside = x >= first[0] && x < first[0] + n && y >= first[1] && y < first[1] + n;
            let (mut r, to) = (run(i), run(i + 1));
            while r < to {
                let (z, len, value) = crate::ways::run_of(run(RUNS_RUN_AT + r));
                let air = value & crate::ways::AIR_OF != 0;
                if inside || air {
                    let d = if air { 1 } else { 0 };
                    (lo, hi) = (lo.min((z - d) * s), hi.max((z + len as i32 + d) * s));
                }
                r += 1;
            }
            x += 1;
        }
        y += 1;
    }
    crate::ground::bound_word(if lo > hi { [i16::MAX as i32, i16::MIN as i32] } else { [lo, hi] })
}

/// Whether the descent asks the ways at block `c` of `zoom`, its column's
/// window `f`: where its bin's word (`bin_word`) meets the block, or its
/// cell is on its square's edge, past which the fill sees no air. The edge
/// goes with the rebake that widens a line's footprint to its reach and
/// past (`ways::footprint`), so a square's lines carry the ring past it and
/// its fill folds the ring's air into the edge bins' words.
pub fn asks(w: &impl Ran, f: &Found, zoom: Zoom, c: [i32; 3]) -> bool {
    if !f.here {
        return false;
    }
    let i = square_of(cell_at(zoom, f.from, [c[0], c[1]])).1;
    let (x, y) = (i % SQUARE, i / SQUARE);
    if x == 0 || y == 0 || x == SQUARE - 1 || y == SQUARE - 1 {
        return true;
    }
    let [lo, hi] = crate::ground::bound_of(w.way(f.from, f.slot, (y >> WAYS_SHIFT) * WAYS_BINS_ACROSS + (x >> WAYS_SHIFT)) as i32);
    // The block's leaf cells.
    let l = crate::LEAF_CELL;
    let (zlo, zhi) = if zoom <= l {
        (c[2] << (l - zoom), (c[2] + 1) << (l - zoom))
    } else {
        (c[2] >> (zoom - l), (c[2] >> (zoom - l)) + 1)
    };
    zlo < hi && zhi > lo
}

/// The finest window, at `cell` or coarser to `top`, holding cell `g` (at
/// `cell`, X named on any lap round the world) present: that window's zoom,
/// the slot and the cell's index in its square; false where none does, a
/// lack.
pub fn find(w: &impl Squares, cell: Zoom, g: [i32; 2], top: Zoom) -> (Zoom, u32, u32, bool) {
    let mut c = cell;
    loop {
        let s = cell - c;
        let (q, i) = square_of([wrap_index(c, g[0] >> s), g[1] >> s]);
        let (slot, here) = w.present(c, q);
        if here {
            return (c, slot, i, true);
        }
        if c <= top {
            return (c, 0, 0, false);
        }
        c -= 1;
    }
}

/// Entry `at` of level `j` of a square, over the level under it, written
/// before: the join of the four under it, at level 1 four cells' reach.
pub fn join(w: &impl Squares, cell: Zoom, slot: u32, j: u32, [x, y]: [u32; 2]) -> i32 {
    let under = |q: u32| {
        let (cx, cy) = (2 * x + q % 2, 2 * y + q / 2);
        if j == 1 {
            reach(w, cell, slot, cy * SQUARE + cx)
        } else {
            crate::ground::bound_of(w.level(cell, slot, j - 1, [cx, cy]))
        }
    };
    let [mut lo, mut hi] = under(0);
    let mut q = 1;
    while q < 4 {
        let [l, h] = under(q);
        (lo, hi) = (lo.min(l), hi.max(h));
        q += 1;
    }
    crate::ground::bound_word([lo, hi])
}

/// Levels a window bounds a node over, its squares' own and then theirs
/// joined, to the window whole.
pub const WINDOW_LEVELS: u32 = LEVELS + ACROSS.trailing_zeros();

/// The lowest and highest the cells at `cell` reach under the node
/// `(zoom, c)`: off the finest window holding them present (`find`), one
/// entry of its square's levels, or the cell holding a node finer than a
/// cell; and that window's zoom. A node wider than a square is the join of
/// its squares' whole, each held at `cell`. None (`ground::UNBOUNDED`) for a
/// node wider than a window, or over a square not held at `cell`, which
/// reads nothing and only splits more; false where no window holds it, a
/// lack.
pub fn bounded(w: &impl Squares, cell: Zoom, (zoom, c): (Zoom, [i32; 2]), top: Zoom) -> ([i32; 2], Zoom, bool) {
    // The node's first cell at `cell`, or the cell holding it.
    let g = if zoom <= cell {
        [c[0] << (cell - zoom), c[1] << (cell - zoom)]
    } else {
        [c[0] >> (zoom - cell), c[1] >> (zoom - cell)]
    };
    if cell > zoom + LEVELS {
        return (whole(w, cell, g, cell - zoom), cell, true);
    }
    let (from, slot, i, here) = find(w, cell, g, top);
    if !here {
        return ([0, 0], from, false);
    }
    if zoom >= from {
        return (reach(w, from, slot, i), from, true);
    }
    let j = from - zoom;
    let at = [(i % SQUARE) >> j, (i / SQUARE) >> j];
    (crate::ground::bound_of(w.level(from, slot, j, at)), from, true)
}

/// The join of the squares at `cell` a node `j` levels over a cell covers,
/// from the one holding its first cell `g`, each its whole; none
/// (`ground::UNBOUNDED`) past the window's levels or where one is not held.
fn whole(w: &impl Squares, cell: Zoom, g: [i32; 2], j: u32) -> [i32; 2] {
    if j > WINDOW_LEVELS {
        return crate::ground::UNBOUNDED;
    }
    let (q, _) = square_of([wrap_index(cell, g[0]), g[1]]);
    let n: u32 = 1 << (j - LEVELS);
    let [mut lo, mut hi] = [i32::MAX, i32::MIN];
    let mut k: u32 = 0;
    while k < n * n {
        let (slot, here) = w.present(cell, [wrap_index(squares_zoom(cell), q[0] + (k % n) as i32), q[1] + (k / n) as i32]);
        if !here {
            return crate::ground::UNBOUNDED;
        }
        let [l, h] = crate::ground::bound_of(w.level(cell, slot, LEVELS, [0, 0]));
        (lo, hi) = (lo.min(l), hi.max(h));
        k += 1;
    }
    [lo, hi]
}
