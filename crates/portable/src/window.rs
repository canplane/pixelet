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

use crate::lattice::TILE_CELLS;
use crate::{wrap_index, Zoom};

/// A square's side in cells, as a shift.
pub const SQUARE_SHIFT: u32 = 6;
pub const SQUARE: u32 = 1 << SQUARE_SHIFT;
pub const SQUARE_CELLS: u32 = SQUARE * SQUARE;
/// Squares a window holds a side, and in all.
pub const ACROSS: u32 = 4;
pub const SLOTS: u32 = ACROSS * ACROSS;
/// A window's side in cells.
pub const WINDOW: u32 = ACROSS * SQUARE;

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

/// The ways' bound a tile holds (pxl-spec bake/tile/terrain.rs `ways_of`):
/// by bins `WAYS_ACROSS` a side, each `1 << WAYS_SHIFT` cells, then its
/// levels up to the tile whole, each the join of the four under it, a bound
/// word an entry (`ground::bound_word`); an empty one `[i16::MAX, i16::MIN]`,
/// which a join passes over.
pub const WAYS_ACROSS: u32 = 32;
pub const WAYS_SHIFT: u32 = (TILE_CELLS / WAYS_ACROSS).trailing_zeros();

/// Where level `k` of a bound `across` bins a side begins, the bins level
/// 0, in entries.
pub const fn ways_at(across: u32, k: u32) -> u32 {
    let n = across * across;
    4 * (n - (n >> (2 * k))) / 3
}

/// A square's share of its tile's ways' bound: its bins and their levels up
/// to the square whole, cut out of the tile's where it fills.
pub const WAYS_LEVELS: u32 = SQUARE_SHIFT - WAYS_SHIFT;
const WAYS_BINS_ACROSS: u32 = SQUARE >> WAYS_SHIFT;
pub const WAYS_BOUND: u32 = ways_at(WAYS_BINS_ACROSS, WAYS_LEVELS + 1);

/// A square's words on a GPU, its lanes in order: its four looks a cell, its
/// heights two a word, its levels, its ways' bound, then its tile's padded
/// samples, two a word. Its heights, levels and ways' bound follow one
/// another, read back whole.
pub const LOOK_WORDS: u32 = SQUARE_CELLS;
pub const HEIGHT_WORDS: u32 = SQUARE_CELLS / 2;
pub const ELE_WORDS: u32 = crate::ground::ELE_WORDS;
pub const SQUARE_WORDS: u32 = LOOK_WORDS + HEIGHT_WORDS + LEVEL_WORDS + WAYS_BOUND + ELE_WORDS;
pub const HEIGHTS_AT: u32 = LOOK_WORDS;
pub const LEVELS_AT: u32 = HEIGHTS_AT + HEIGHT_WORDS;
pub const WAYS_AT: u32 = LEVELS_AT + LEVEL_WORDS;
pub const ELE_AT: u32 = WAYS_AT + WAYS_BOUND;
/// What comes back of a square, from `HEIGHTS_AT`.
pub const BACK_WORDS: u32 = HEIGHT_WORDS + LEVEL_WORDS + WAYS_BOUND;

/// The square at `place` (in squares) of a tile's ways' bound `tile` (an
/// entry by index), its own entries in `out`, a bound word each.
pub fn square_ways(tile: impl Fn(u32) -> [i32; 2], place: [u32; 2], out: &mut [u32]) {
    let mut k = 0;
    while k <= WAYS_LEVELS {
        let side = WAYS_BINS_ACROSS >> k;
        let mut y = 0;
        while y < side {
            let mut x = 0;
            while x < side {
                let from = ways_at(WAYS_ACROSS, k) + (place[1] * side + y) * (WAYS_ACROSS >> k) + place[0] * side + x;
                out[(ways_at(WAYS_BINS_ACROSS, k) + y * side + x) as usize] = crate::ground::bound_word(tile(from)) as u32;
                x += 1;
            }
            y += 1;
        }
        k += 1;
    }
}

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
    /// Entry `k` of the ways' bound of the square in `slot`, a bound word
    /// (`WAYS_BOUND`).
    fn way(&self, cell: Zoom, slot: u32, k: u32) -> u32;
    /// Entry `at` of level `j` of the square in `slot`, as a bound word
    /// (`ground::bound_word`): its cells' reach joined (`join`).
    fn level(&self, cell: Zoom, slot: u32, j: u32, at: [u32; 2]) -> i32;
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
