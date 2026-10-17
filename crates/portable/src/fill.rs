// pxl-portable/fill.rs

//! gpu-portable: a square's fill, a thread at a time, over the buffer its
//! words lie in from `base` (`window::SQUARE_WORDS`): each column's ground
//! top off its tile's samples and its cell as laid (`heights`), its ways'
//! runs off its lines in three passes (`ways`, a workgroup's scan between
//! them, `stretch_runs` and `scanned`), and its levels, level 0 each column's own
//! (`level`). A GPU runs each pass a thread a column or entry
//! (gpu-query-shader `heights_cs`, `ways_cs`, `bound_cs`); a bake runs its
//! columns' passes in order over one square (`square`), so the runs it
//! sizes a tile's room by are the ones a GPU writes.

use crate::area::Shard;
use crate::window::{
    self, column_first, column_top, Squares, COLUMNS_AT, ELE_AT, FRESH_CELL, FRESH_FIRST, FRESH_PLACE, FRESH_SHARD, FRESH_SQUARE, FRESH_WORDS, LAID_AT, LEVELS_AT, LEVEL_ENTRY_WORDS, LINES_AT,
    RUNS_AT, RUNS_SQUARE, SQUARE, SQUARE_CELLS, SQUARE_WORDS,
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

/// The samples handed the square at `base` (`ELE_AT`), two a word, low
/// first, each its half's sign carried up: `ground::height`'s.
fn samples(ground: &[u32], base: u32) -> impl Fn(usize) -> i32 + '_ {
    move |i: usize| ((at(ground, base + ELE_AT + i as u32 / 2) >> (16 * (i as u32 % 2))) << 16) as i32 >> 16
}

/// A square being filled, on its own: what `level` folds.
struct Fresh<'a> {
    ground: &'a [u32],
    base: u32,
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
    fn look(&self, _: u32, _: u32, i: u32) -> u32 {
        at(self.ground, self.base + i)
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

/// A square being filled, as the ways' rule reads it (`window::Square`):
/// its lines, laid cells and heights in its lane, and where it is
/// as handed (`window::FRESH_WORDS`).
struct Filling<'a> {
    ground: &'a [u32],
    base: u32,
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
        at(self.ground, self.base + LINES_AT + k)
    }
    fn laid(&self, i: u32) -> u32 {
        laid_of(self.ground, self.base, i)
    }
    /// Off its tile's samples as `heights` draws a cell (`ground::height`),
    /// the ring about the square too.
    fn height(&self, c: [i32; 2]) -> i32 {
        let first = [(self.place[0] * SQUARE) as i32, (self.place[1] * SQUARE) as i32];
        ground::height(samples(self.ground, self.base), [first[0] + c[0], first[1] + c[1]])
    }
}

/// Column `[x, y]` of the square `fresh` hands (`window::fresh`): its
/// ground's top (`ground::height`) off its tile's samples, a gone cell's
/// (its own look `UNKNOWN`, past the area) the Earth's at sea level, so it
/// reads as the Earth (`area::laid`); its runs' first the ways' to write
/// (`ways`); and, a column of an even `x`, its cell and
/// the next as laid, the word they share (`window::LAID_WORDS`), so each
/// word has one writer: its own look off its four at the square's zoom
/// (`ground::own`), and at a leaf's cells or finer, which of the leaf
/// cells about its own are past the area (`area::gone_ring`), off the
/// square's shard.
pub fn heights(ground: &mut [u32], fresh: [u32; FRESH_WORDS as usize], [x, y]: [u32; 2]) {
    let word = |k: u32| fresh[k as usize];
    let (base, place, cell) = (word(FRESH_SQUARE) * SQUARE_WORDS, window::place_of(word(FRESH_PLACE)), word(FRESH_CELL));
    let (first, shard) = ([word(FRESH_FIRST) as i32, word(FRESH_FIRST + 1) as i32], Shard::of_word(word(FRESH_SHARD)));
    let i = window::index_of([x, y]);
    let own = ground::own(at(ground, base + i), cell);
    let top = if own == ground::UNKNOWN {
        crate::SEA_LEVEL
    } else {
        let lane: &[u32] = ground;
        ground::height(samples(lane, base), [(place[0] * SQUARE + x) as i32, (place[1] * SQUARE + y) as i32])
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
    let next = ground::own(at(ground, base + i + 1), cell);
    put(ground, base + LAID_AT + i / 2, window::laid_half(own, gone0) | window::laid_half(next, gone1) << 16);
}

/// The ways of the square `fresh` hands (`window::fresh`), for column `i`:
/// unless `write`, its runs counted into its column word's first; where
/// `write`, once each column's first is where its runs begin (`scanned`),
/// its runs written from it — each off its spans and those beside it
/// (`ways::around`) read along z (`ways::runs`). A column's spans,
/// `u32::MAX` where its or those beside it do not fit `ways::SPANS`; 0 for
/// the rest.
pub fn ways(ground: &mut [u32], fresh: [u32; FRESH_WORDS as usize], write: bool, i: u32) -> u32 {
    let word = |k: u32| fresh[k as usize];
    let (base, place, cell) = (word(FRESH_SQUARE) * SQUARE_WORDS, window::place_of(word(FRESH_PLACE)), word(FRESH_CELL));
    let columns = base + COLUMNS_AT;
    if i >= SQUARE_CELLS {
        return 0;
    }
    let (around, fits) = {
        let lane: &[u32] = ground;
        let filling = Filling { ground: lane, base, cell, place };
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

/// Thread `t`'s stretch of the square `fresh` hands: its columns' runs, as
/// counted into their words' firsts (`ways`), summed.
pub fn stretch_runs(ground: &[u32], fresh: [u32; FRESH_WORDS as usize], t: u32) -> u32 {
    let columns = fresh[FRESH_SQUARE as usize] * SQUARE_WORDS + COLUMNS_AT + t * STRETCH;
    let (mut n, mut k) = (0, 0);
    while k < STRETCH {
        (n, k) = (n + column_first(at(ground, columns + k)), k + 1);
    }
    n
}

/// Thread `t`'s stretch of the square `fresh` hands, its columns' firsts
/// written where their runs begin, `before` the runs of the stretches
/// before it (an exclusive scan of `stretch_runs` over the threads,
/// Blelloch 1990): each column's the runs before it; the last thread's,
/// the square's count in the word past them (`window::COLUMN_WORDS`).
pub fn scanned(ground: &mut [u32], fresh: [u32; FRESH_WORDS as usize], t: u32, before: u32) {
    let columns = fresh[FRESH_SQUARE as usize] * SQUARE_WORDS + COLUMNS_AT;
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
/// what its runs write fills, a bore's air none (`ground::Bounds`); a higher
/// one over the level under it, written before (`window::join`).
pub fn level(ground: &mut [u32], base: u32, cell: Zoom, j: u32, at: [u32; 2]) {
    let w = if j == 0 {
        let i = window::index_of(at);
        let (w, to) = (column_of(ground, base, i), column_first(column_of(ground, base, i + 1)));
        let (top, s) = (column_top(w), 1i32 << (crate::LEAF_CELL - cell));
        let ([mut lo, mut hi], mut r) = (ground::EMPTY, column_first(w));
        while r + 1 < to {
            let (z, value) = ways::run_of(crate::at(ground, base + RUNS_AT + r));
            if value != ways::NOTHING && value != ways::AIR {
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
/// gpu/renderer.rs `grids`): `lane` its words, its looks, samples and lines
/// as handed; its cells at `cell`, lying at `place` in its tile; its
/// columns, the levels no bake reads; inside the area, so no wall cuts its
/// ways and the runs are the most a client's fill writes. The runs its ways
/// read as, and the most spans a column makes (`u32::MAX` past
/// `ways::SPANS`).
pub fn square(lane: &mut [u32], place: [u32; 2], cell: Zoom) -> [u32; 2] {
    let fresh = window::fresh(0, window::place_word(place), cell, [0; 2], Shard::INSIDE);
    for y in 0..SQUARE {
        for x in 0..SQUARE {
            heights(lane, fresh, [x, y]);
        }
    }
    let mut most = 0;
    for i in 0..SQUARE_CELLS {
        most = most.max(ways(lane, fresh, false, i));
    }
    let mut before = 0;
    for t in 0..SCAN_THREADS {
        let n = stretch_runs(lane, fresh, t);
        scanned(lane, fresh, t, before);
        before += n;
    }
    for i in 0..SQUARE_CELLS {
        ways(lane, fresh, true, i);
    }
    [column_first(column_of(lane, 0, SQUARE_CELLS)), most]
}
