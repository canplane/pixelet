// pxl-portable/fill.rs

//! gpu-portable: a square's fill, a thread at a time, off its record in the
//! frame's fresh list (`window::HANDED_WORDS`, its words, looks, samples and
//! lines) into the buffer its words lie in from `base`
//! (`window::SQUARE_WORDS`): each column's ground
//! top off its tile's samples and its cell as laid (`heights`), its ways'
//! runs off its lines in three passes (`ways`, a workgroup's scan between
//! them, `stretch_runs` and `scanned`), and its levels, level 0 each column's own
//! (`level`). A GPU runs each pass a thread a column or entry
//! (gpu-query-shader `heights_cs`, `ways_cs`, `bound_cs`); a bake runs its
//! columns' passes in order over one square (`square`), so the runs it
//! sizes a tile's room by are the ones a GPU writes.

use crate::area::Shard;
use crate::window::{
    self, column_first, column_top, handed_at, Squares, COLUMNS_AT, ELE_AT, FRESH_CELL, FRESH_FIRST, FRESH_PLACE, FRESH_SHARD, FRESH_SQUARE, FRESH_WORDS, LAID_AT, LEVELS_AT, LEVEL_ENTRY_WORDS,
    LINES_AT, LOOKS_AT, RUNS_AT, RUNS_SQUARE, SQUARE, SQUARE_CELLS, SQUARE_WORDS,
};
use crate::{at, ground, ways, Zoom};

/// Word `i` of `s` set to `v`, as `at` reads: totally, none past it, so no
/// store aborts.
pub fn put(s: &mut [u32], i: u32, v: u32) {
    if (i as usize) < s.len() {
        // SAFETY: just bounded by the slice's own length.
        unsafe { *s.get_unchecked_mut(i as usize) = v };
    }
}

/// Column word `i` off the square at `base` (`window::COLUMN_WORDS`).
pub fn column_of(ground: &[u32], base: u32, i: u32) -> u32 {
    at(ground, base + COLUMNS_AT + i)
}

/// Entry `at` of level `j` off the square at `base`, its two bound words
/// (`window::LEVEL_ENTRY_WORDS`).
pub fn level_of(ground: &[u32], base: u32, j: u32, [x, y]: [u32; 2]) -> [i32; 2] {
    let e = base + LEVELS_AT + LEVEL_ENTRY_WORDS * window::level_index(j, [x, y]);
    [at(ground, e) as i32, at(ground, e + 1) as i32]
}

/// Cell `i` as laid off the square at `base` (`window::LAID_WORDS`).
pub fn laid_of(ground: &[u32], base: u32, i: u32) -> u32 {
    window::laid_at(at(ground, base + LAID_AT + i / 2), i)
}

/// The samples handed in the record at `rec` of a fresh list (`ELE_AT`),
/// two a word, low first, each its half's sign carried up:
/// `ground::height`'s.
fn samples(fresh: &[u32], rec: u32) -> impl Fn(usize) -> i32 + '_ {
    move |i: usize| ((at(fresh, rec + ELE_AT + i as u32 / 2) >> (16 * (i as u32 % 2))) << 16) as i32 >> 16
}

/// The handed square `k` of a fresh list: its record's first word, its
/// square's first, where it lies in its tile and its cells' zoom.
fn handed(fresh: &[u32], k: u32) -> (u32, u32, [u32; 2], Zoom) {
    let rec = handed_at(k);
    (
        rec,
        at(fresh, rec + FRESH_SQUARE) * SQUARE_WORDS,
        window::place_of(at(fresh, rec + FRESH_PLACE)),
        at(fresh, rec + FRESH_CELL),
    )
}

/// A square being filled, on its own: what `level` folds.
pub struct Fresh<'a> {
    pub ground: &'a [u32],
    pub base: u32,
}

impl Squares for Fresh<'_> {
    fn present(&self, _: u32, _: [i32; 2]) -> (u32, bool) {
        (0, true)
    }
    /// A square on its own is no window's.
    fn square(&self, _: u32, _: u32) -> ([i32; 2], bool) {
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
}

impl ways::tree::Runs for Fresh<'_> {
    fn column(&self, i: u32) -> [u32; 2] {
        [column_first(column_of(self.ground, self.base, i)), column_first(column_of(self.ground, self.base, i + 1))]
    }
    fn run(&self, r: u32) -> u32 {
        at(self.ground, self.base + RUNS_AT + r)
    }
}

/// The square handed `k`-th in `fresh` as its ways' tree is of: its cells'
/// zoom and its index, its first cell's over its side (`ways::tree::Of`).
pub fn of(fresh: &[u32], k: u32) -> ways::tree::Of {
    let rec = handed_at(k);
    let first = [at(fresh, rec + FRESH_FIRST) as i32, at(fresh, rec + FRESH_FIRST + 1) as i32];
    ways::tree::Of {
        cell: at(fresh, rec + FRESH_CELL),
        at: [first[0] >> window::SQUARE_SHIFT, first[1] >> window::SQUARE_SHIFT],
    }
}

/// A square being filled, as the ways' rule reads it (`window::Square`):
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

impl window::Square for Filling<'_> {
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
    /// A column of the square's own as `heights` wrote it (`column_top`),
    /// read and not drawn again; the ring about it, which no column word
    /// holds, off its tile's samples as `heights` draws a cell
    /// (`ground::height`).
    fn height(&self, c: [i32; 2]) -> i32 {
        let side = SQUARE as i32;
        if c[0] >= 0 && c[1] >= 0 && c[0] < side && c[1] < side {
            return column_top(column_of(self.ground, self.base, window::index_of([c[0] as u32, c[1] as u32])));
        }
        let first = [(self.place[0] * SQUARE) as i32, (self.place[1] * SQUARE) as i32];
        ground::height(samples(self.fresh, self.rec), [first[0] + c[0], first[1] + c[1]])
    }
}

/// Column `[x, y]` of the square a frame hands `k`-th in `fresh`: its
/// ground's top (`ground::height`) off its tile's samples, a gone cell's
/// (its own look `UNKNOWN`, past the area) the Earth's at sea level, so it
/// reads as the Earth (`area::laid`); its runs' first the ways' to write
/// (`ways`); and, a column of an even `x`, its cell and
/// the next as laid, the word they share (`window::LAID_WORDS`), so each
/// word has one writer: its own look off its four at the square's zoom
/// (`ground::own`), and at a leaf's cells or finer, which of the leaf
/// cells about its own are past the area (`area::gone_ring`), off the
/// square's shard.
pub fn heights(ground: &mut [u32], fresh: &[u32], k: u32, [x, y]: [u32; 2]) {
    let (rec, base, place, cell) = handed(fresh, k);
    let (first, shard) = (
        [at(fresh, rec + FRESH_FIRST) as i32, at(fresh, rec + FRESH_FIRST + 1) as i32],
        Shard::of_word(at(fresh, rec + FRESH_SHARD)),
    );
    let i = window::index_of([x, y]);
    let own = ground::own(at(fresh, rec + LOOKS_AT + i), cell);
    let top = if own == ground::UNKNOWN {
        crate::SEA_LEVEL
    } else {
        ground::height(samples(fresh, rec), [(place[0] * SQUARE + x) as i32, (place[1] * SQUARE + y) as i32])
    };
    put(ground, base + COLUMNS_AT + i, top as u32 & 0xffff);
    if x % 2 == 1 {
        return;
    }
    // The leaf cells gone about each: finer than the leaves, the pair lies
    // in one leaf cell, whose ring is both's.
    let gone = |k: u32| {
        let b = [crate::wrap_index(cell, first[0] + (x + k) as i32), first[1] + y as i32];
        let s = cell - crate::LEAF_CELL;
        crate::area::gone_ring(shard, (cell, b), [b[0] >> s, b[1] >> s])
    };
    let gone0 = if cell >= crate::LEAF_CELL { gone(0) } else { 0 };
    let gone1 = if cell == crate::LEAF_CELL { gone(1) } else { gone0 };
    let next = ground::own(at(fresh, rec + LOOKS_AT + i + 1), cell);
    put(ground, base + LAID_AT + i / 2, window::laid_half(own, gone0) | window::laid_half(next, gone1) << 16);
}

/// Cell `[x, y]`'s four looks of the square a frame hands `k`-th in
/// `fresh`, as handed (`LOOKS_AT`), into `looks`, a word a cell,
/// `SQUARE_CELLS` a square in the squares' order: what a ray's hit is
/// drawn by (`ground::Cell::four`), the one read of them past the fill.
pub fn look(looks: &mut [u32], fresh: &[u32], k: u32, [x, y]: [u32; 2]) {
    let rec = window::handed_at(k);
    let i = window::index_of([x, y]);
    put(looks, at(fresh, rec + FRESH_SQUARE) * SQUARE_CELLS + i, at(fresh, rec + LOOKS_AT + i));
}

/// The ways of the square a frame hands `k`-th in `fresh`, for column `i`:
/// unless `write`, its runs counted into its column word's first; where
/// `write`, once each column's first is where its runs begin (`scanned`),
/// its runs written from it — each off its spans and those beside it
/// (`ways::around`) read along z (`ways::runs`). A column's spans,
/// `u32::MAX` where its or those beside it do not fit `ways::SPANS`; 0 for
/// the rest.
pub fn ways(ground: &mut [u32], fresh: &[u32], k: u32, write: bool, i: u32) -> u32 {
    let (rec, base, place, cell) = handed(fresh, k);
    let columns = base + COLUMNS_AT;
    if i >= SQUARE_CELLS {
        return 0;
    }
    let (around, fits) = {
        let lane: &[u32] = ground;
        let filling = Filling {
            ground: lane,
            base,
            fresh,
            rec,
            cell,
            place,
        };
        let [x, y] = window::of_index(i);
        crate::ways::around(&filling, [x as i32, y as i32])
    };
    let w = at(ground, columns + i);
    let (from, room) = if write { (column_first(w), RUNS_SQUARE) } else { (0, 0) };
    let n = put_runs(ground, base, &around, cell, from, room);
    if !write {
        put(ground, columns + i, with_first(w, n));
    }
    if fits {
        around.spans[0].n
    } else {
        u32::MAX
    }
}

/// The threads that sum a square's columns' counts into their firsts, a
/// workgroup (gpu-query-shader `ways_cs`), each over a stretch of the
/// columns in their order.
pub const SCAN_THREADS: u32 = 64;
const STRETCH: u32 = SQUARE_CELLS / SCAN_THREADS;

/// Thread `t`'s stretch of the square handed `k`-th in `fresh`: its columns' runs, as
/// counted into their words' firsts (`ways`), summed.
pub fn stretch_runs(ground: &[u32], fresh: &[u32], k: u32, t: u32) -> u32 {
    let columns = handed(fresh, k).1 + COLUMNS_AT + t * STRETCH;
    let (mut n, mut k) = (0, 0);
    while k < STRETCH {
        (n, k) = (n + column_first(at(ground, columns + k)), k + 1);
    }
    n
}

/// Thread `t`'s stretch of the square handed `k`-th in `fresh`, its columns' firsts
/// written where their runs begin, `before` the runs of the stretches
/// before it (an exclusive scan of `stretch_runs` over the threads,
/// Blelloch 1990): each column's the runs before it; the last thread's,
/// the square's count in the word past them (`window::COLUMN_WORDS`).
pub fn scanned(ground: &mut [u32], fresh: &[u32], k: u32, t: u32, before: u32) {
    let columns = handed(fresh, k).1 + COLUMNS_AT;
    let (mut n, mut k) = (before, t * STRETCH);
    while k < (t + 1) * STRETCH {
        let w = at(ground, columns + k);
        put(ground, columns + k, with_first(w, n));
        (n, k) = (n + column_first(w), k + 1);
    }
    if t + 1 == SCAN_THREADS {
        put(ground, columns + SQUARE_CELLS, with_first(0, n));
    }
}

const _: () = assert!(STRETCH * SCAN_THREADS == SQUARE_CELLS, "the stretches cover the columns");

/// Column word `w` with its first run `n`, its top kept.
fn with_first(w: u32, n: u32) -> u32 {
    w & 0xffff | n << 16
}

/// A column's runs off its spans and those beside it (`ways::runs`) into
/// the square at `base`'s runs from `first`, as far as `room`; how many
/// there are.
fn put_runs(ground: &mut [u32], base: u32, around: &ways::Around, cell: u32, first: u32, room: u32) -> u32 {
    ways::runs(around, cell, move |k, w| {
        if first + k < room {
            put(ground, base + RUNS_AT + first + k, w);
        }
    })
}

/// Entry `[x, y]` of level `j` of the square at `base`, its cells at `cell`:
/// level 0 column `[x, y]`'s own — its ground's top, and the leaf cells
/// its runs write (`ground::Bounds`); a higher one over the level under it,
/// written before (`window::join`). A bore's air counts, as the ways write
/// it: the blocks of the square's ways tree lie in the bound
/// (`ways::tree`), and a node it splits more holds only air or void there,
/// a bore lying under its column's face.
pub fn level(ground: &mut [u32], base: u32, cell: Zoom, j: u32, at: [u32; 2]) {
    let w = if j == 0 {
        let i = window::index_of(at);
        let (w, to) = (column_of(ground, base, i), column_first(column_of(ground, base, i + 1)));
        let (top, s) = (column_top(w), 1i32 << (crate::LEAF_CELL - cell));
        let ([mut lo, mut hi], mut r) = (ground::EMPTY, column_first(w));
        while r + 1 < to {
            let (z, value) = ways::run_of(crate::at(ground, base + RUNS_AT + r));
            if value != ways::NOTHING {
                let end = ways::run_of(crate::at(ground, base + RUNS_AT + r + 1)).0;
                (lo, hi) = (lo.min(z * s), hi.max(end * s));
            }
            r += 1;
        }
        ground::bounds_word([[top, top], [lo, hi]])
    } else {
        let under: &[u32] = ground;
        window::join(&Fresh { ground: under, base }, 0, 0, j, at)
    };
    let e = base + LEVELS_AT + LEVEL_ENTRY_WORDS * window::level_index(j, at);
    put(ground, e, w[0] as u32);
    put(ground, e + 1, w[1] as u32);
}

/// A square filled whole, its passes in a GPU's order (pxl-render
/// gpu/renderer.rs `grids`): `fresh` its record (`window::HANDED_WORDS`),
/// its looks, samples and lines as handed, its words laid here; `lane` its
/// words; its cells at `cell`, lying at `place` in its tile, its first cell
/// `first`; its columns and levels; inside the area, so no wall cuts its
/// ways and the runs are the most a client's fill writes. The runs its ways
/// read as, and the most spans a column makes (`u32::MAX` past
/// `ways::SPANS`); its ways' tree is built off them after (`tree`).
pub fn square(fresh: &mut [u32], lane: &mut [u32], place: [u32; 2], cell: Zoom, first: [i32; 2]) -> [u32; 2] {
    fresh[..FRESH_WORDS as usize].copy_from_slice(&window::fresh(0, window::place_word(place), cell, first, Shard::INSIDE));
    let fresh: &[u32] = fresh;
    for y in 0..SQUARE {
        for x in 0..SQUARE {
            heights(lane, fresh, 0, [x, y]);
        }
    }
    let mut most = 0;
    for i in 0..SQUARE_CELLS {
        most = most.max(ways(lane, fresh, 0, false, i));
    }
    let mut before = 0;
    for t in 0..SCAN_THREADS {
        let n = stretch_runs(lane, fresh, 0, t);
        scanned(lane, fresh, 0, t, before);
        before += n;
    }
    for i in 0..SQUARE_CELLS {
        ways(lane, fresh, 0, true, i);
    }
    for j in 0..=window::LEVELS {
        let side = SQUARE >> j;
        for k in 0..side * side {
            level(lane, 0, cell, j, [k % side, k / side]);
        }
    }
    [column_first(column_of(lane, 0, SQUARE_CELLS)), most]
}

/// The square handed first in `fresh`, filled into `lane` (`square`), its
/// ways' tree built into `pool`, its heap words then its data words, a
/// level at a time as a GPU's workgroup builds it (gpu-query-shader
/// `ways_tree_cs`, `ways::tree`), its blocks taken in order: the blocks it
/// took, and those it holds once folded; `u32::MAX` both where `pool` has
/// too few or it takes more than `ways::tree::SQUARE_BLOCKS`.
pub fn tree(lane: &[u32], fresh: &[u32], pool: &mut [u32]) -> [u32; 2] {
    use ways::tree::{self as t, Lane};
    let (of, square) = (of(fresh, 0), Fresh { ground: lane, base: 0 });
    let room = pool.len() as u32 / 16;
    let mut p = Lane(pool);
    // Block 0 is none's: the root's is the first handed out.
    t::root(&mut p, of, 8);
    // Each level's candidates, the blocks the level over it took.
    let mut levels = [[0u32; 2]; crate::LEAF_CELL as usize + 1];
    let (mut b, mut n, mut taken) = (8, 8, 1);
    let mut zoom = 0;
    while zoom < of.cell {
        levels[zoom as usize] = [b, n];
        let flagged = (b..b + n).filter(|&s| t::flag(&p, &square, 0, of, zoom, s)).count() as u32;
        if !t::fits(room - 1 - taken, taken, flagged) {
            return [u32::MAX; 2];
        }
        let next = 8 * (1 + taken);
        let mut k = 0;
        for s in b..b + n {
            if t::flag(&p, &square, 0, of, zoom, s) {
                t::branch(&mut p, of, zoom, s, next + 8 * k);
                k += 1;
            } else {
                t::empty(&mut p, s);
            }
        }
        (b, n, taken, zoom) = (next, 8 * flagged, taken + flagged, zoom + 1);
    }
    for s in b..b + n {
        t::leaf(&mut p, &square, of, s);
    }
    let mut freed = 0;
    while zoom > 0 {
        zoom -= 1;
        let [b, n] = levels[zoom as usize];
        for s in b..b + n {
            if t::under(&p, s) != t::NONE && t::fold(&mut p, zoom, s) != t::NONE {
                freed += 1;
            }
        }
    }
    [taken, taken - freed]
}
