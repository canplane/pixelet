// pxl-portable/fill.rs

//! gpu-portable: a square's fill, a thread at a time, over the buffer its
//! words lie in from `base` (`window::SQUARE_WORDS`): its cells' heights off
//! its tile's samples (`heights`), its ways' runs off its lines in four
//! passes (`ways`), and its levels over its cells' reach (`level`). A GPU
//! runs each pass a thread a word, column or entry (gpu-query-shader
//! `heights_cs`, `ways_cs`, `bound_cs`); a bake runs the same passes in
//! order over one square (`square`), so the bound it keeps of a tile is the
//! one a GPU draws of it.

use crate::window::{self, Squares, ELE_AT, HEIGHTS_AT, LEVELS, LEVELS_AT, LINES_AT, RUNS_AT, RUNS_RUN_AT, RUNS_SQUARE, SQUARE, SQUARE_CELLS, WAYS_AT, WAYS_BINS};
use crate::{at, ground, ways, Zoom};

/// Word `i` of `s` set to `v`, as `at` reads: totally, none past it, so no
/// store aborts.
pub fn put(s: &mut [u32], i: u32, v: u32) {
    if (i as usize) < s.len() {
        // SAFETY: just bounded by the slice's own length.
        unsafe { *s.get_unchecked_mut(i as usize) = v };
    }
}

/// Cell `i`'s height off the square at `base`: two a word, low first, the
/// half's sign carried up.
pub fn height_of(ground: &[u32], base: u32, i: u32) -> i32 {
    ((at(ground, base + HEIGHTS_AT + i / 2) >> (16 * (i % 2))) << 16) as i32 >> 16
}

/// Entry `at` of level `j` off the square at `base`.
pub fn level_of(ground: &[u32], base: u32, j: u32, [x, y]: [u32; 2]) -> i32 {
    at(ground, base + LEVELS_AT + window::level_at(j) + y * (SQUARE >> j) + x) as i32
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
    fn look(&self, _: u32, _: u32, i: u32) -> u32 {
        at(self.ground, self.base + i)
    }
    fn height(&self, _: u32, _: u32, i: u32) -> i32 {
        height_of(self.ground, self.base, i)
    }
    fn way(&self, _: u32, _: u32, k: u32) -> u32 {
        at(self.ground, self.base + WAYS_AT + k)
    }
    fn level(&self, _: u32, _: u32, j: u32, at: [u32; 2]) -> i32 {
        level_of(self.ground, self.base, j, at)
    }
}

/// A square being filled, as the ways' rule reads it (`window::Square`):
/// its lines, looks and heights in its lane.
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
    fn looks(&self, i: u32) -> u32 {
        at(self.ground, self.base + i)
    }
    /// Off its tile's samples as `heights` draws a cell (`ground::height`),
    /// the ring about the square too.
    fn height(&self, c: [i32; 2]) -> i32 {
        let (samples, base) = (self.ground, self.base);
        let ele = move |i: usize| ((at(samples, base + ELE_AT + i as u32 / 2) >> (16 * (i as u32 % 2))) << 16) as i32 >> 16;
        let first = [(self.place[0] * SQUARE) as i32, (self.place[1] * SQUARE) as i32];
        let center = crate::lattice::CELL_CENTER;
        ground::bicubic(ele, (first[0] + c[0]) as f32 + center, (first[1] + c[1]) as f32 + center)
    }
}

/// Word `[x, y]` of the square at `base`'s heights, two cells
/// (`ground::height`) off its tile's samples, the square lying at `place`
/// in it, in squares.
pub fn heights(ground: &mut [u32], base: u32, place: [u32; 2], [x, y]: [u32; 2]) {
    let first = [place[0] * SQUARE, place[1] * SQUARE];
    let w = {
        let samples: &[u32] = ground;
        let ele = move |i: usize| ((at(samples, base + ELE_AT + i as u32 / 2) >> (16 * (i as u32 % 2))) << 16) as i32 >> 16;
        let (cx, cy) = (first[0] + 2 * x, first[1] + y);
        let (a, b) = (ground::height(ele, [cx, cy]), ground::height(ele, [cx + 1, cy]));
        (a as u32 & 0xffff) | (b as u32) << 16
    };
    put(ground, base + HEIGHTS_AT + y * (SQUARE / 2) + x, w);
}

/// Pass `pass` of the square at `base`'s ways, its cells at `cell`, at
/// `place` in its tile, for column or bin `i`: 0 counts each column's runs
/// into its table word, 1 (`i` 0 alone) sums them in order into each cell's
/// first, 2 writes each column's runs from its first, 3 writes each bin's
/// ways' word off them (`window::walk_ways`, `ways::runs`), which the levels
/// join after. A column's spans, `u32::MAX` where they do not fit
/// `ways::SPANS`; 0 for the rest.
pub fn ways(ground: &mut [u32], base: u32, place: [u32; 2], cell: Zoom, pass: u32, i: u32) -> u32 {
    let table = base + RUNS_AT;
    if pass == 1 {
        if i == 0 {
            let (mut n, mut k) = (0, 0);
            while k < SQUARE_CELLS {
                let count = at(ground, table + k);
                put(ground, table + k, n);
                (n, k) = (n + count, k + 1);
            }
            put(ground, table + SQUARE_CELLS, n);
        }
        return 0;
    }
    let (bins, writes) = (pass == 3, pass == 2);
    if i >= if bins { WAYS_BINS } else { SQUARE_CELLS } {
        return 0;
    }
    let (spans, word, fits) = {
        let lane: &[u32] = ground;
        let filling = Filling { ground: lane, base, cell, place };
        window::walk_ways(&filling, move |k: u32| at(lane, table + k), bins, i)
    };
    if bins {
        put(ground, base + WAYS_AT + i, word as u32);
        return 0;
    }
    let (first, room) = if writes { (at(ground, table + i), RUNS_SQUARE) } else { (0, 0) };
    let n = put_runs(ground, base, &spans, cell, first, room);
    if !writes {
        put(ground, table + i, n);
    }
    if fits {
        spans.n
    } else {
        u32::MAX
    }
}

/// A column's runs off its spans (`ways::runs`) into the square at `base`'s
/// runs lane from `first`, as far as `room`; how many there are.
fn put_runs(ground: &mut [u32], base: u32, spans: &ways::Spans, cell: u32, first: u32, room: u32) -> u32 {
    ways::runs(spans, cell, move |k, w| {
        if first + k < room {
            put(ground, base + RUNS_AT + RUNS_RUN_AT + first + k, w);
        }
    })
}

/// Entry `[x, y]` of level `j` of the square at `base`, over the level under
/// it, written before (`window::join`); level 1 over its cells' reach.
pub fn level(ground: &mut [u32], base: u32, j: u32, at: [u32; 2]) {
    let w = {
        let under: &[u32] = ground;
        window::join(&Fresh { ground: under, base }, 0, 0, j, at)
    };
    put(ground, base + LEVELS_AT + window::level_at(j) + at[1] * (SQUARE >> j) + at[0], w as u32);
}

/// A square filled whole, its passes in a GPU's order (pxl-render
/// gpu/renderer.rs `grids`): `lane` its words, its looks, samples and lines
/// as handed; its cells at `cell`, lying at `place` in its tile. What it
/// reaches is its last level's one entry (`bound`). The runs its ways read
/// as, and the most spans a column makes (`u32::MAX` past `ways::SPANS`).
pub fn square(lane: &mut [u32], place: [u32; 2], cell: Zoom) -> [u32; 2] {
    for y in 0..SQUARE {
        for x in 0..SQUARE / 2 {
            heights(lane, 0, place, [x, y]);
        }
    }
    let mut most = 0;
    for pass in 0..4 {
        let n = match pass {
            1 => 1,
            3 => WAYS_BINS,
            _ => SQUARE_CELLS,
        };
        for i in 0..n {
            most = most.max(ways(lane, 0, place, cell, pass, i));
        }
    }
    for j in 1..=LEVELS {
        for y in 0..SQUARE >> j {
            for x in 0..SQUARE >> j {
                level(lane, 0, j, [x, y]);
            }
        }
    }
    [at(lane, RUNS_AT + SQUARE_CELLS), most]
}

/// The lowest and highest a filled square's cells reach, in leaf cells:
/// its last level, whole.
pub fn bound(lane: &[u32]) -> [i32; 2] {
    ground::bound_of(level_of(lane, 0, LEVELS, [0, 0]))
}
