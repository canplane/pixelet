// pxl-portable/fill.rs

//! gpu-portable: a square's fill, a thread at a time, over the buffer its
//! words lie in from `base` (`window::SQUARE_WORDS`): each column's ground
//! top off its tile's samples (`heights`), its ways' runs off its lines in
//! three passes (`ways`), and its levels, level 0 each column's own
//! (`level`). A GPU runs each pass a thread a column or entry
//! (gpu-query-shader `heights_cs`, `ways_cs`, `bound_cs`); a bake runs its
//! columns' passes in order over one square (`square`), so the runs it
//! sizes a tile's room by are the ones a GPU writes.

use crate::area::Shard;
use crate::window::{
    self, column_first, column_top, Squares, COLUMNS_AT, ELE_AT, FRESH_CELL, FRESH_FIRST, FRESH_PLACE, FRESH_SHARD, FRESH_SQUARE, FRESH_WORDS, LEVELS_AT, LINES_AT, RUNS_AT, RUNS_SQUARE, SQUARE,
    SQUARE_CELLS, SQUARE_WORDS,
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

/// Entry `at` of level `j` off the square at `base`.
pub fn level_of(ground: &[u32], base: u32, j: u32, [x, y]: [u32; 2]) -> i32 {
    at(ground, base + LEVELS_AT + window::level_index(j, [x, y])) as i32
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
    fn level(&self, _: u32, _: u32, j: u32, at: [u32; 2]) -> i32 {
        level_of(self.ground, self.base, j, at)
    }
}

/// A square being filled, as the ways' rule reads it (`window::Square`):
/// its lines, looks and heights in its lane, and where it is as handed
/// (`window::FRESH_WORDS`).
struct Filling<'a> {
    ground: &'a [u32],
    base: u32,
    cell: u32,
    place: [u32; 2],
    first: [i32; 2],
    shard: Shard,
}

impl window::Square for Filling<'_> {
    fn cell(&self) -> u32 {
        self.cell
    }
    fn place(&self) -> [u32; 2] {
        self.place
    }
    fn first(&self) -> [i32; 2] {
        self.first
    }
    fn shard(&self) -> Shard {
        self.shard
    }
    fn line(&self, k: u32) -> u32 {
        at(self.ground, self.base + LINES_AT + k)
    }
    fn looks(&self, i: u32) -> u32 {
        at(self.ground, self.base + i)
    }
    /// Off its tile's samples as `heights` draws a cell (`ground::height`),
    /// the ring about the square too.
    fn height(&self, c: [i32; 2]) -> i32 {
        let first = [(self.place[0] * SQUARE) as i32, (self.place[1] * SQUARE) as i32];
        ground::height(samples(self.ground, self.base), [first[0] + c[0], first[1] + c[1]])
    }
}

/// Column `[x, y]` of the square at `base`: its ground's top
/// (`ground::height`) off its tile's samples, the square lying at `place`
/// in it, in squares; its runs' first is the ways' to write (`ways`).
pub fn heights(ground: &mut [u32], base: u32, place: [u32; 2], [x, y]: [u32; 2]) {
    let w = {
        let lane: &[u32] = ground;
        ground::height(samples(lane, base), [(place[0] * SQUARE + x) as i32, (place[1] * SQUARE + y) as i32]) as u32 & 0xffff
    };
    put(ground, base + COLUMNS_AT + y * SQUARE + x, w);
}

/// Pass `pass` of the ways of the square `fresh` hands (`window::fresh`),
/// for column `i`: 0 counts its runs into its column
/// word's first, 1 (`i` 0 alone) sums them in order into each column's
/// first and the last word's, 2 writes its runs from its first, each off
/// its spans (`ways::column`) read along z (`ways::runs`). A column's spans,
/// `u32::MAX` where they do not fit `ways::SPANS`; 0 for the rest.
pub fn ways(ground: &mut [u32], fresh: [u32; FRESH_WORDS as usize], pass: u32, i: u32) -> u32 {
    let word = |k: u32| fresh[k as usize];
    let (base, place, cell) = (word(FRESH_SQUARE) * SQUARE_WORDS, window::place_of(word(FRESH_PLACE)), word(FRESH_CELL));
    let (first, shard) = ([word(FRESH_FIRST) as i32, word(FRESH_FIRST + 1) as i32], Shard::of_word(word(FRESH_SHARD)));
    let columns = base + COLUMNS_AT;
    if pass == 1 {
        if i == 0 {
            let (mut n, mut k) = (0, 0);
            while k < SQUARE_CELLS {
                let w = at(ground, columns + k);
                put(ground, columns + k, with_first(w, n));
                (n, k) = (n + column_first(w), k + 1);
            }
            put(ground, columns + SQUARE_CELLS, with_first(0, n));
        }
        return 0;
    }
    if i >= SQUARE_CELLS {
        return 0;
    }
    let (spans, fits) = {
        let lane: &[u32] = ground;
        let filling = Filling {
            ground: lane,
            base,
            cell,
            place,
            first,
            shard,
        };
        crate::ways::column(&filling, [(i % SQUARE) as i32, (i / SQUARE) as i32])
    };
    let w = at(ground, columns + i);
    let (from, room) = if pass == 2 { (column_first(w), RUNS_SQUARE) } else { (0, 0) };
    let n = put_runs(ground, base, &spans, cell, from, room);
    if pass == 0 {
        put(ground, columns + i, with_first(w, n));
    }
    if fits {
        spans.n
    } else {
        u32::MAX
    }
}

/// Column word `w` with its first run `n`, its top kept.
fn with_first(w: u32, n: u32) -> u32 {
    w & 0xffff | n << 16
}

/// A column's runs off its spans (`ways::runs`) into the square at `base`'s
/// runs from `first`, as far as `room`; how many there are.
fn put_runs(ground: &mut [u32], base: u32, spans: &ways::Spans, cell: u32, first: u32, room: u32) -> u32 {
    ways::runs(spans, cell, move |k, w| {
        if first + k < room {
            put(ground, base + RUNS_AT + first + k, w);
        }
    })
}

/// Entry `[x, y]` of level `j` of the square at `base`, its cells at `cell`:
/// level 0 column `[x, y]`'s own — its ground's top joined with what its
/// runs write, a way's air a block either side of it, as `ways::block`
/// recolours over and under it; a higher one over the level under it,
/// written before (`window::join`).
pub fn level(ground: &mut [u32], base: u32, cell: Zoom, j: u32, at: [u32; 2]) {
    let w = if j == 0 {
        let i = at[1] * SQUARE + at[0];
        let (w, to) = (column_of(ground, base, i), column_first(column_of(ground, base, i + 1)));
        let (top, s) = (column_top(w), 1i32 << (crate::LEAF_CELL - cell));
        let (mut lo, mut hi, mut r) = (top, top, column_first(w));
        while r + 1 < to {
            let (z, value) = ways::run_of(crate::at(ground, base + RUNS_AT + r));
            if value != ways::NOTHING {
                let d = if value & ways::AIR_OF != 0 { 1 } else { 0 };
                let end = ways::run_of(crate::at(ground, base + RUNS_AT + r + 1)).0;
                (lo, hi) = (lo.min((z - d) * s), hi.max((end + d) * s));
            }
            r += 1;
        }
        ground::bound_word([lo, hi])
    } else {
        let under: &[u32] = ground;
        window::join(&Fresh { ground: under, base }, 0, 0, j, at)
    };
    put(ground, base + LEVELS_AT + window::level_index(j, at), w as u32);
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
            heights(lane, 0, place, [x, y]);
        }
    }
    let mut most = 0;
    for pass in 0..3 {
        for i in 0..if pass == 1 { 1 } else { SQUARE_CELLS } {
            most = most.max(ways(lane, fresh, pass, i));
        }
    }
    [column_first(column_of(lane, 0, SQUARE_CELLS)), most]
}
