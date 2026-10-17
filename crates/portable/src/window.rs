// pxl-portable/window.rs

//! gpu-portable: the ground's cells as they are held, a window a zoom about
//! a centre. A ray observes a zoom's cells only within `RADIUS` of them
//! from the eye (pxl-spec space/layer/demand.rs `band`, the top's by its
//! reach), so a window `WINDOW` cells a side about it holds every cell it
//! can observe there. A window is `ACROSS` squares a side, each square
//! held in the slot its place names modulo `ACROSS`, so the window follows
//! its centre a square at a time and no held cell moves. A square is
//! present once its cells are drawn; a reader of one absent reads the
//! coarser window's (`find`).

use crate::ground::{ELE_WORDS, UNKNOWN, VOID};
use crate::lattice::{GROUND, TILE_CELLS};
use crate::{wrap_index, Zoom, LEAF_CELL};

/// A square's side in cells, as a shift.
pub const SQUARE_SHIFT: u32 = 6;
pub const SQUARE: u32 = 1 << SQUARE_SHIFT;
pub const SQUARE_CELLS: u32 = SQUARE * SQUARE;
/// Squares a window holds a side, and in all: as many as hold `RADIUS`
/// about a centre anywhere in a square, to a power of two, as `slot` masks
/// by it.
pub const ACROSS: u32 = ((2 * RADIUS + SQUARE).div_ceil(SQUARE)).next_power_of_two();
pub const SLOTS: u32 = ACROSS * ACROSS;
/// A window's side in cells.
pub const WINDOW: u32 = ACROSS * SQUARE;
/// The eye's coarsest window's cells' zoom: the top's.
pub const FIRST_CELL: Zoom = GROUND.cell_zoom(GROUND.top());
/// The eye's windows, every zoom the pyramid draws, coarsest first, which
/// the GPU's cast reads: the one at `cell` is `cell − FIRST_CELL`.
pub const EYE_WINDOWS: u32 = LEAF_CELL - FIRST_CELL + 1;

/// The cells' zoom of the eye's window `w`, coarsest first: what the layout
/// lays it at and the GPU's decor reads a square's window at.
pub const fn eye_cell(w: u32) -> Zoom {
    FIRST_CELL + w
}
/// Every window: the eye's; the focus's, the leaves' and one over, what
/// walking and editing read; a destination's one, at the zoom its line says
/// its ground, found exactly (pxl-kernel layer/terrain/window.rs).
pub const WINDOWS: u32 = EYE_WINDOWS + 3;
/// Squares every window holds, the eye's first: the most the GPU holds, and
/// so the most of their cells read back at once.
pub const SQUARES: u32 = WINDOWS * SLOTS;
/// What the GPU's fill costs a frame, measured at dpr 2 on an M-series Mac
/// (2026-10-17, the `fill` span): its passes whatever it is handed, and each
/// square more; handed one square a frame it took 2.2 ms, sixteen 5.5 ms.
pub const FILL_MS: f32 = 2.0;
pub const FILL_SQUARE_MS: f32 = 0.22;
/// The share of a frame the fill may take.
pub const FILL_SHARE_MS: f32 = 4.0;
/// The squares a frame hands the fill at most, nearest first (pxl-kernel
/// layer/terrain/window.rs `due`): its share less its passes, over a square.
pub const HANDED: u32 = ((FILL_SHARE_MS - FILL_MS) / FILL_SQUARE_MS) as u32;

/// A square handed to the GPU's fill, in words: its index, where it lies in
/// its tile (`place_word`, in squares), its cells' zoom, its window's as the
/// window holds it (pxl-kernel layer/terrain/window.rs `Window::cell`; a
/// destination's moves with its line), never derived from the index; its
/// first cell at that zoom as the world names it, X wrapped there, then Y,
/// two words; and what is known of the service area at it (`area::Shard`,
/// its word), which the ways' fill lays the area's edge by
/// (`ways::Columns::walled`).
pub const FRESH_SQUARE: u32 = 0;
pub const FRESH_PLACE: u32 = FRESH_SQUARE + 1;
pub const FRESH_CELL: u32 = FRESH_PLACE + 1;
pub const FRESH_FIRST: u32 = FRESH_CELL + 1;
pub const FRESH_SHARD: u32 = FRESH_FIRST + 2;
pub const FRESH_WORDS: u32 = FRESH_SHARD + 1;

/// A fresh square's words (`FRESH_WORDS`), `place` its `place_word`.
pub const fn fresh(square: u32, place: u32, cell: Zoom, first: [i32; 2], shard: crate::area::Shard) -> [u32; FRESH_WORDS as usize] {
    let mut w = [0; FRESH_WORDS as usize];
    (w[FRESH_SQUARE as usize], w[FRESH_PLACE as usize], w[FRESH_CELL as usize]) = (square, place, cell);
    (w[FRESH_FIRST as usize], w[FRESH_FIRST as usize + 1], w[FRESH_SHARD as usize]) = (first[0] as u32, first[1] as u32, shard.word());
    w
}

/// Where a square lies in its tile, in squares, as one word: X low, Y high.
pub const fn place_word([x, y]: [u32; 2]) -> u32 {
    x | y << 16
}

/// `place_word`'s place.
pub const fn place_of(w: u32) -> [u32; 2] {
    [w & 0xffff, w >> 16]
}

/// How far from its centre a window holds every cell, in its own cells: a
/// zoom's band ends `SPLIT` of its cells from the eye (`cast::SPLIT`).
pub const RADIUS: u32 = crate::cast::SPLIT as u32;

/// The first square of the window at `cell` about the cell `c`:
/// ⌊(c − RADIUS) / SQUARE⌋ a side, X wrapped round the world. Its squares
/// hold the cells [c − RADIUS, c + RADIUS] wherever in a square `c` lies.
pub fn origin(cell: Zoom, c: [i32; 2]) -> [i32; 2] {
    let first = |v: i32| (v - RADIUS as i32) >> SQUARE_SHIFT;
    [wrap_index(squares_zoom(cell), first(c[0])), first(c[1])]
}

/// The zoom a window's squares are counted at: its cells' over `SQUARE_SHIFT`.
pub const fn squares_zoom(cell: Zoom) -> Zoom {
    cell - SQUARE_SHIFT
}

/// The slot square `q` is held in.
pub fn slot(q: [i32; 2]) -> u32 {
    let m = (ACROSS - 1) as i32;
    ((q[1] & m) as u32) * ACROSS + (q[0] & m) as u32
}

/// The square holding cell `g`, and the cell's index in it.
pub fn square_of(g: [i32; 2]) -> ([i32; 2], u32) {
    let m = (SQUARE - 1) as i32;
    ([g[0] >> SQUARE_SHIFT, g[1] >> SQUARE_SHIFT], ((g[1] & m) as u32) * SQUARE + (g[0] & m) as u32)
}

/// Where a square's levels begin among them, in entries: level `j`, from 0,
/// each entry the lowest and highest of the 2^j × 2^j cells under it — level
/// 0 a cell's own column (`fill::level`).
pub const fn level_at(j: u32) -> u32 {
    (4 * SQUARE_CELLS - ((4 * SQUARE_CELLS) >> (2 * j))) / 3
}

/// Entry `[x, y]` of level `j` among a square's levels, row by row.
pub const fn level_index(j: u32, [x, y]: [u32; 2]) -> u32 {
    level_at(j) + y * (SQUARE >> j) + x
}
pub const LEVELS: u32 = SQUARE_SHIFT;
pub const LEVEL_WORDS: u32 = level_at(LEVELS + 1);

/// A tile's ways are cut into bins `WAYS_ACROSS` a side, each
/// `1 << WAYS_SHIFT` cells: a square lays its lines by them (`LINES_AT`),
/// which its fill reads its columns' runs off.
pub const WAYS_ACROSS: u32 = 32;
pub const WAYS_SHIFT: u32 = (TILE_CELLS / WAYS_ACROSS).trailing_zeros();
const WAYS_BINS_ACROSS: u32 = SQUARE >> WAYS_SHIFT;
pub const WAYS_BINS: u32 = WAYS_BINS_ACROSS * WAYS_BINS_ACROSS;

/// The most entries a square holds of its tile's ways: one a segment and a
/// bin (`WAYS_SHIFT`) its footprint reaches. The densest square
/// of the format-12 set, z15 010332102333322 (2026-10-17; z14's most 711);
/// a tile past it fails its bake (pxl-spec bake/tile/terrain.rs `ways_fit`).
pub const WAYS_SQUARE_SEGMENTS: u32 = 1065;

/// A square's lines, laid by the CPU as the square is filled (pxl-spec
/// bake/tile/terrain.rs `square_lines`) for its fill to write its columns'
/// runs off (`fill::ways`): its tile's header (`ways::header`), a word a bin
/// (`WAYS_BINS`), its first entry and its count over it; the entries, two a
/// word, each one of the square's segments, a bin's in their order; then
/// the segments a bin reaches, in their lines' order, `ways::SEGMENT_WORDS`
/// each. None holds more than `WAYS_SQUARE_SEGMENTS` of either.
pub const LINES_ENTRIES_AT: u32 = crate::ways::HEADER_WORDS + WAYS_BINS;
pub const LINES_SEGMENTS_AT: u32 = LINES_ENTRIES_AT + WAYS_SQUARE_SEGMENTS.div_ceil(2);
pub const LINES_WORDS: u32 = LINES_SEGMENTS_AT + WAYS_SQUARE_SEGMENTS * crate::ways::SEGMENT_WORDS as u32;

/// The most runs a square's columns read as (`ways::runs`): the most of the
/// format-12 set (2026-10-17), measured with every tile at every zoom (z15's
/// 8600; z14's most 3674); a tile past it fails its bake (pxl-spec
/// bake/tile/terrain.rs `runs_fit`).
pub const RUNS_SQUARE: u32 = 8600;

/// A square's columns, a word a cell, row by row, and one more after the
/// last: a cell's ground's top in leaf cells, an `i16`, low (`column_top`),
/// and its first run among the square's runs, high (`column_first`), so a
/// cell's runs are its first to the next cell's; the last word's first is
/// how many there are. A column is its ground's top and the runs the ways
/// write over it along z (`ways::run_word`), lowest first, each to the next's
/// first block and a column's last `NOTHING` on; a column no way writes in
/// has none.
pub const COLUMN_WORDS: u32 = SQUARE_CELLS + 1;

/// A column word's ground top.
pub const fn column_top(w: u32) -> i32 {
    (w << 16) as i32 >> 16
}

/// A column word's first run.
pub const fn column_first(w: u32) -> u32 {
    w >> 16
}

/// A square's cells as its fill lays them, two a word, cell `i` in the
/// `i % 2`-th half (`laid_at`): its own look, low (`ground::own` of its four
/// at the square's zoom), and which of the eight leaf cells about it are
/// past the area, high (`area::gone_ring`, none at a square coarser than a
/// leaf's): what a read of it lays the area's edge by (`area::laid`),
/// never working out its majority or its ring again.
pub const LAID_WORDS: u32 = SQUARE_CELLS / 2;

/// Cell `i`'s half of a laid word `w`.
pub const fn laid_at(w: u32, i: u32) -> u32 {
    (w >> (16 * (i % 2))) & 0xffff
}

/// A cell's laid half: its own look and the leaf cells about it gone.
pub const fn laid_half(look: u32, gone: u32) -> u32 {
    look & 0xff | gone << 8
}

/// A square's words on a GPU, its lanes in order: its four looks a cell, its
/// columns, its levels, its columns' runs, its cells as laid, then its
/// tile's padded samples, two a word, and its lines, which only its fill
/// reads. Its columns, levels, runs and laid cells follow one another, read
/// back whole.
pub const LOOK_WORDS: u32 = SQUARE_CELLS;
pub const SQUARE_WORDS: u32 = LOOK_WORDS + COLUMN_WORDS + LEVEL_WORDS + RUNS_SQUARE + LAID_WORDS + ELE_WORDS + LINES_WORDS;
pub const COLUMNS_AT: u32 = LOOK_WORDS;
pub const LEVELS_AT: u32 = COLUMNS_AT + COLUMN_WORDS;
pub const RUNS_AT: u32 = LEVELS_AT + LEVEL_WORDS;
pub const LAID_AT: u32 = RUNS_AT + RUNS_SQUARE;
pub const ELE_AT: u32 = LAID_AT + LAID_WORDS;
pub const LINES_AT: u32 = ELE_AT + ELE_WORDS;
/// What comes back of a square, from `COLUMNS_AT`: its columns, levels,
/// runs and laid cells, so the CPU reads a column as the GPU does, never
/// working it out again.
pub const BACK_WORDS: u32 = COLUMN_WORDS + LEVEL_WORDS + RUNS_SQUARE + LAID_WORDS;

/// The windows as one side holds them: per window, a zoom's cells about
/// its centre, which square each slot holds once present, and its cells.
pub trait Squares {
    /// Where square `q` at `cell` is held present: a slot of the side's own
    /// naming, which the reads below take; false where no window holds it.
    fn present(&self, cell: Zoom, q: [i32; 2]) -> (u32, bool);
    /// The square the eye's window at `cell` holds in `slot`, and whether
    /// present: `present` the other way.
    fn square(&self, cell: Zoom, slot: u32) -> ([i32; 2], bool);
    /// The four looks of cell `i` of the square in `slot` (`ground::looks`).
    fn look(&self, cell: Zoom, slot: u32, i: u32) -> u32;
    /// Column word `i` of the square in `slot` (`COLUMN_WORDS`), `i` to
    /// `SQUARE_CELLS`.
    fn column(&self, cell: Zoom, slot: u32, i: u32) -> u32;
    /// Entry `at` of level `j` of the square in `slot`, as a bound word
    /// (`ground::bound_word`): its columns' joined (`join`), level 0 one.
    fn level(&self, cell: Zoom, slot: u32, j: u32, at: [u32; 2]) -> i32;
    /// Cell `i`'s ground's top, in leaf cells, as the GPU drew it.
    fn height(&self, cell: Zoom, slot: u32, i: u32) -> i32 {
        column_top(self.column(cell, slot, i))
    }
    /// Cell `i` as its fill laid it (`LAID_WORDS`, `laid_at`).
    fn laid(&self, cell: Zoom, slot: u32, i: u32) -> u32;
    /// Entry `e` of the eye's window at `cell`'s levels over its squares
    /// (`JOINED_WORDS`, `fold`), as the GPU folded them; the zero entry,
    /// none, where a side holds no fold.
    fn joined(&self, cell: Zoom, e: u32) -> [u32; JOINED_WORDS as usize];
}

/// One square as its fill reads it, for the ways' one rule
/// (`ways::Columns`): its cells' zoom, where it lies in its tile (in
/// squares), word `k` of its lines (`LINES_AT`), cell `i`'s four looks as
/// its lane holds them and as `heights` laid it (`LAID_WORDS`), the area's
/// edge it is walled by, and column `c`'s height off its tile's samples, `c`
/// counted from its first cell, the ring of columns about it included.
/// Its columns are its cells and that ring, a ring column read off the
/// edge bin it borders, whose lines hold every one that reaches it
/// (`ways::footprint`); one past the ring has no segment.
pub trait Square {
    fn cell(&self) -> Zoom;
    fn place(&self) -> [u32; 2];
    fn line(&self, k: u32) -> u32;
    fn looks(&self, i: u32) -> u32;
    /// Cell `i` as its fill laid it (`LAID_WORDS`), before its ways: what
    /// they wall it by.
    fn laid(&self, i: u32) -> u32;
    fn height(&self, c: [i32; 2]) -> i32;
}

/// Whether column `c` is the square's or its ring's.
fn held(c: [i32; 2]) -> bool {
    c[0] >= -1 && c[1] >= -1 && c[0] <= SQUARE as i32 && c[1] <= SQUARE as i32
}

/// Column `c`'s cell index in the square, none outside it.
fn cell_index(c: [i32; 2]) -> u32 {
    if c[0] < 0 || c[1] < 0 || c[0] >= SQUARE as i32 || c[1] >= SQUARE as i32 {
        return SQUARE_CELLS;
    }
    c[1] as u32 * SQUARE + c[0] as u32
}

/// Column `c`'s bin word, a ring column's the edge bin it borders: its
/// first entry and its count; none past the ring.
fn bin_of(sq: &impl Square, c: [i32; 2]) -> (u32, u32) {
    if !held(c) {
        return (0, 0);
    }
    let (x, y) = (c[0].clamp(0, SQUARE as i32 - 1) as u32, c[1].clamp(0, SQUARE as i32 - 1) as u32);
    let b = (y >> WAYS_SHIFT) * WAYS_BINS_ACROSS + (x >> WAYS_SHIFT);
    let w = sq.line(crate::ways::HEADER_WORDS + b);
    (w & 0xffff, w >> 16)
}

/// Entry `e` of the square's lines: the place of a segment among its.
fn entry_of(sq: &impl Square, e: u32) -> u32 {
    (sq.line(LINES_ENTRIES_AT + e / 2) >> (16 * (e % 2))) & 0xffff
}

fn segment_at(sq: &impl Square, at: u32) -> crate::ways::Segment {
    let base = LINES_SEGMENTS_AT + at * crate::ways::SEGMENT_WORDS as u32;
    let mut w = [0; crate::ways::SEGMENT_WORDS];
    let mut k = 0;
    while k < w.len() {
        w[k] = sq.line(base + k as u32);
        k += 1;
    }
    crate::ways::Segment::of(w)
}

fn header_of(sq: &impl Square) -> (f32, crate::ways::Stations) {
    let mut w = [0; crate::ways::HEADER_WORDS as usize];
    let mut k = 0;
    while k < w.len() {
        w[k] = sq.line(k as u32);
        k += 1;
    }
    crate::ways::of_header(w)
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
        let center = crate::lattice::CELL_CENTER;
        [(p[0] * SQUARE) as f32 + c[0] as f32 + center, (p[1] * SQUARE) as f32 + c[1] as f32 + center]
    }
    /// Off its cell as laid (`area::laid`), as a read of it walls it: a
    /// square's first cell lies on a leaf cell's first, so `c` places the
    /// column in its leaf cell as its world index does. A ring column is
    /// none of the square's, never walled.
    fn walled(&self, c: [i32; 2]) -> bool {
        let i = cell_index(c);
        i != SQUARE_CELLS && crate::area::laid(self.laid(i), Square::cell(self), c) == UNKNOWN
    }
    fn side(&self, _: [i32; 2]) -> f32 {
        1.0
    }
    fn stations(&self, _: [i32; 2]) -> crate::ways::Stations {
        header_of(self).1
    }
    /// Its own look, before the service area's wall is laid: the wall is the
    /// descent's to lay (`ran`'s reader).
    fn look(&self, c: [i32; 2]) -> u32 {
        let i = cell_index(c);
        if i == SQUARE_CELLS {
            VOID
        } else {
            crate::ground::own(self.looks(i), Square::cell(self))
        }
    }
    fn face(&self, c: [i32; 2]) -> i32 {
        if !held(c) {
            return 0;
        }
        let cell = Square::cell(self);
        crate::ground::face_height(cell, crate::ground::blocks(cell, self.height(c) as f32))
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

/// What the ways' runs read of a side's windows beyond their columns: run
/// `r` of the square in `slot` at `cell` (`RUNS_AT`).
pub trait Ran: Squares {
    fn ran(&self, cell: Zoom, slot: u32, r: u32) -> u32;
}

/// Where a node's cells at `cell`, a square's or fewer, are held, as `find`
/// finds them: the zoom they were asked at, the window holding them present
/// (its zoom `from`, at `cell` or coarser, and its slot), or none; one word,
/// as a descent carries it down beside what is placed (`placed::Held`).
/// `find` reads only the square at `cell` holding them and the squares over
/// it, so every node whose cells lie in that square has this answer: a
/// descent hands it from a node to the nodes under it (`step`). `NONE` was
/// asked at no zoom.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Found(u32);

/// A slot as a source names it: a GPU's in the eye's window at `from`, the
/// CPU's among every window's (`SQUARES`).
const FOUND_SLOT_BITS: u32 = SQUARES.next_power_of_two().trailing_zeros();
const FOUND_HERE: u32 = 1 << FOUND_SLOT_BITS;
const FOUND_FROM_SHIFT: u32 = FOUND_SLOT_BITS + 1;
const FOUND_CELL_SHIFT: u32 = FOUND_FROM_SHIFT + crate::ZOOM_BITS;
const FOUND_ZOOM_MASK: u32 = (1 << crate::ZOOM_BITS) - 1;
/// A `Found`'s word's bits: its slot, here, and its two zooms.
pub const FOUND_BITS: u32 = FOUND_CELL_SHIFT + crate::ZOOM_BITS;
const _: () = assert!(FOUND_BITS <= 32);
const _: () = assert!(FIRST_CELL >= 2, "a zoom asked is never NONE's, nor one finer");

impl Found {
    pub const NONE: Found = Found(0);

    const fn of(cell: Zoom, from: Zoom, slot: u32, here: bool) -> Found {
        Found(cell << FOUND_CELL_SHIFT | from << FOUND_FROM_SHIFT | (here as u32) << FOUND_SLOT_BITS | slot)
    }
    /// The zoom its cells were asked at.
    pub const fn cell(self) -> Zoom {
        self.0 >> FOUND_CELL_SHIFT
    }
    /// The window's zoom.
    pub const fn from(self) -> Zoom {
        (self.0 >> FOUND_FROM_SHIFT) & FOUND_ZOOM_MASK
    }
    pub const fn slot(self) -> u32 {
        self.0 & (FOUND_HERE - 1)
    }
    pub const fn here(self) -> bool {
        self.0 & FOUND_HERE != 0
    }
    pub const fn word(self) -> u32 {
        self.0
    }
    pub const fn of_word(w: u32) -> Found {
        Found(w)
    }
}

/// The cell at `from` column `c` of blocks at `zoom` lies in.
fn cell_at(zoom: Zoom, from: Zoom, c: [i32; 2]) -> [i32; 2] {
    let s = zoom - from;
    [wrap_index(from, c[0] >> s), c[1] >> s]
}

/// The window holding cell `g` at `cell`, as `find` finds it
/// (`Found`), and the cell's index in its square there.
pub fn found_at(w: &impl Squares, cell: Zoom, g: [i32; 2]) -> (Found, u32) {
    let (from, slot, i, here) = find(w, cell, g);
    (Found::of(cell, from, slot, here), i)
}

/// `found_at` for a node's cells at `cell` from its first, `g`, stepped
/// from `above`, what a node over it found, or `NONE`, reading only what
/// `above` leaves unknown. Only a node a square wide or less sets a place
/// (`bounded`), so every node a descent asks under it, a sibling from its
/// parent's stack entry too, lies in that node, and at each zoom to
/// `above`'s cell in the square that held `above`'s cells there. `find`
/// asks those zooms finest first and stops at the first holding them, so
/// `above` says what each of them answers: none present over `above`'s
/// window, it present. Asked at `above`'s cell or coarser down to its
/// window, the answer is that window, read nowhere; coarser than the window,
/// `found_at`. Asked finer, only the zooms finer than `above`'s are unknown:
/// they are looked at finest first, then `above`'s answer stands, a look a
/// zoom of the step.
pub fn step(w: &impl Squares, above: Found, cell: Zoom, g: [i32; 2]) -> (Found, u32) {
    if above == Found::NONE || cell < above.from() {
        return found_at(w, cell, g);
    }
    let mut f = Found::of(cell, above.from(), above.slot(), above.here());
    let mut c = cell;
    while c > above.cell() {
        let (slot, here) = w.present(c, square_of(cell_at(cell, c, g)).0);
        if here {
            f = Found::of(cell, c, slot, true);
            break;
        }
        c -= 1;
    }
    (f, square_of(cell_at(cell, f.from(), g)).1)
}

/// What the ways write in block `z` of the column `c` of blocks at `zoom`,
/// as its window holds it run along z (`ways::at`'s): the block it lies in
/// there, off its cell, past the leaves a leaf's. A column whose cell lies
/// in the square holding `own`'s, which `near` found, is read off it, as
/// the columns about one mostly are (`step`); one past it finds its own.
/// `ways::NOTHING` where no way writes, or no window holds it.
pub fn ran(w: &impl Ran, near: Found, own: [i32; 2], zoom: Zoom, c: [i32; 2], z: i32) -> u32 {
    let read = held_to(zoom);
    let g = cell_at(zoom, read, c);
    let q = square_of(g).0;
    // Component-wise: a GPU has no array equality.
    let above = if q[0] == own[0] && q[1] == own[1] { near } else { Found::NONE };
    let (f, i) = step(w, above, read, g);
    if !f.here() {
        return crate::ways::NOTHING;
    }
    let (from, slot) = (f.from(), f.slot());
    let (zr, to) = (z >> (zoom - from), column_first(w.column(from, slot, i + 1)));
    let (mut r, mut value) = (column_first(w.column(from, slot, i)), crate::ways::NOTHING);
    while r < to {
        let (lo, v) = crate::ways::run_of(w.ran(from, slot, r));
        if zr < lo {
            break;
        }
        (value, r) = (v, r + 1);
    }
    value
}

/// Whether column `c` of blocks at `zoom`, its window `f`, holds a run: a
/// way writes in it.
pub fn runs_in(w: &impl Squares, f: Found, zoom: Zoom, c: [i32; 2]) -> bool {
    if !f.here() {
        return false;
    }
    let (from, slot) = (f.from(), f.slot());
    let i = square_of(cell_at(zoom, from, c)).1;
    column_first(w.column(from, slot, i + 1)) > column_first(w.column(from, slot, i))
}

/// The ground as a side's windows hold it, one reading for the GPU's
/// query (gpu-query-shader `World`) and the CPU's (pxl-kernel
/// layer/terrain/window.rs `Windowed`): the cells, the area's edge laid on
/// them, and the ways their runs write.
pub trait Windows: Ran + crate::area::Area {}

impl<W: Ran + crate::area::Area> Windows for W {}

/// The zoom a cell at `cell` is read at: past the leaves, a leaf's.
pub const fn held_to(cell: Zoom) -> Zoom {
    if cell > LEAF_CELL {
        LEAF_CELL
    } else {
        cell
    }
}

/// The ground's cell `g` at `cell`, off the finest window holding it
/// present, stepped from `place` (`step`): its four looks, its height and
/// where it is held; `void` at sea level where none holds it.
pub fn cell(w: &impl Squares, place: Found, cell: Zoom, g: [i32; 2]) -> (u32, i32, u32, Found) {
    let (f, i) = step(w, place, cell, g);
    if !f.here() {
        return (VOID, 0, laid_half(VOID, 0), f);
    }
    let (from, slot) = (f.from(), f.slot());
    (w.look(from, slot, i), w.height(from, slot, i), w.laid(from, slot, i), f)
}

/// Column `b` at `zoom` as a reader reads it: the cell drawing it, past
/// the leaves a leaf's, stepped from `place`, with the area's edge laid on
/// it off the cell as its fill laid it (`area::laid`); and where it is
/// held. `place` is where the cells of `owner` were found: a column beside it is read off it where its cell lies
/// in the square holding `owner`'s, as `ran` reads the columns about one
/// (`step`), and found anew past it.
pub fn ground(w: &impl Windows, owner: (Zoom, [i32; 2]), place: Found, zoom: Zoom, b: [i32; 2]) -> (crate::ground::Cell, Found) {
    let read = held_to(zoom);
    let g = cell_at(zoom, read, b);
    let (q, o) = (square_of(g).0, square_of(cell_at(owner.0, read, owner.1)).0);
    // Component-wise: a GPU has no array equality.
    let place = if q[0] == o[0] && q[1] == o[1] { place } else { Found::NONE };
    let (word, h, laid, f) = cell(w, place, read, g);
    let cell = crate::ground::Cell {
        look: crate::area::laid(laid, zoom, b),
        top: crate::ground::blocks(zoom, h as f32),
        fold: crate::ground::looks(word),
    };
    (cell, f)
}

/// The ways about block `own` at `zoom` as the windows hold them (`ran`),
/// off the window its ground's cell was read at (`found`) and the columns
/// about it read off it: none in a column past the area or walled, as the
/// fill writes none there (`ways::column`, `ways::Columns::walled`).
/// It holds the windows by value: a GPU holds no pointer in a struct.
struct Over<W> {
    w: W,
    zoom: Zoom,
    /// The square at its cells' zoom its own column's cell lies in.
    square: [i32; 2],
    found: Found,
}

impl<W: Windows> crate::ways::Airs for Over<W> {
    fn at(&self, c: [i32; 2], z: i32) -> u32 {
        ran(&self.w, self.found, self.square, self.zoom, c, z)
    }
}

/// Block `c` at `zoom` with what the ways write over the ground's block
/// there (`ways::block`), `block` its look and its fold's four, `carved`
/// whether a cave made its ground air (`ground::Cell::block`), and `found`
/// where its column was read: air, or a way's look whole.
/// Only a way in the block's own column writes in it, or the air of one
/// beside where the caves carved the ground (`runs_in`). A column read off
/// a coarser window standing in for its own holds that window's runs, laid
/// at its coarser cells, where no wall stands: the coarser zoom's world.
pub fn ways_over(w: &(impl Windows + Copy), found: Found, zoom: Zoom, c: [i32; 3], (block, carved): ((u32, [u32; 4]), bool)) -> (u32, [u32; 4]) {
    if !carved && !runs_in(w, found, zoom, [c[0], c[1]]) {
        return block;
    }
    let over = Over {
        w: *w,
        zoom,
        square: square_of(cell_at(zoom, held_to(zoom), [c[0], c[1]])).0,
        found,
    };
    match crate::ways::block(&over, c, carved) {
        crate::ways::NOTHING => block,
        crate::ways::AIR => (VOID, [VOID; 4]),
        way => (way, [way; 4]),
    }
}

/// The finest window, at `cell` or coarser to the first (`FIRST_CELL`), holding cell `g` (at
/// `cell`, X named on any lap round the world) present: that window's zoom,
/// the slot and the cell's index in its square; false where none does, a
/// lack.
pub fn find(w: &impl Squares, cell: Zoom, g: [i32; 2]) -> (Zoom, u32, u32, bool) {
    let mut c = cell;
    loop {
        let (q, i) = square_of(cell_at(cell, c, g));
        let (slot, here) = w.present(c, q);
        if here {
            return (c, slot, i, true);
        }
        if c <= FIRST_CELL {
            return (c, 0, 0, false);
        }
        c -= 1;
    }
}

/// Entry `at` of level `j` of a square, from 1, over the level under it,
/// written before: the join of the four under it.
pub fn join(w: &impl Squares, cell: Zoom, slot: u32, j: u32, [x, y]: [u32; 2]) -> i32 {
    let under = |q: u32| crate::ground::bound_of(w.level(cell, slot, j - 1, [2 * x + q % 2, 2 * y + q / 2]));
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

/// The levels over a window's squares, from one over a square
/// (`LEVELS + 1`) to the window whole: a node wider than a square bounded
/// by the join of the squares it covers, folded where the windows' slots
/// are written (`fold`) and read as one entry (`bounded`), not joined
/// again at every node that asks.
pub const JOINED_LEVELS: u32 = WINDOW_LEVELS - LEVELS;
/// A window's entries: level `l` over a square has `(ACROSS >> l)²`, an
/// aligned block of `2^l` squares a side each.
pub const JOINED_ENTRIES: u32 = joined_index(JOINED_LEVELS + 1, [0, 0]);
/// An entry's words: its key, the block's first square at its cells' zoom,
/// X then Y, each with its sign flipped (`key_word`), so the zero entry,
/// as fresh memory holds it, names no square; then the join's bound
/// (`ground::bound_word`), and 0: a row of the windows' buffer, an `IVec4`
/// as their slot table's are, after it (`JOINED_AT`).
pub const JOINED_WORDS: u32 = 4;
/// The row of the windows' buffer the eye's windows' entries begin at:
/// past their slot table, a row a slot (pxl-kernel scene/cast.rs `window`).
pub const JOINED_AT: u32 = EYE_WINDOWS * SLOTS;
/// The eye's windows' entries, as the GPU lays them and the CPU reads them
/// back: window `cell − FIRST_CELL` a run of `JOINED_ENTRIES`.
pub const EYE_JOINED_WORDS: u32 = JOINED_ENTRIES * JOINED_WORDS * EYE_WINDOWS;

/// Entry of level `l` (from 1) over a square, its block `at` in blocks of
/// that level, among a window's: the levels' entries in order, a level's
/// row by row.
pub const fn joined_index(l: u32, at: [u32; 2]) -> u32 {
    let mut e = 0;
    let mut m = 1;
    while m < l {
        let across = ACROSS >> m;
        e += across * across;
        m += 1;
    }
    e + at[1] * (ACROSS >> l) + at[0]
}

/// A key's word: `v` with its sign flipped, so no square's is 0.
const fn key_word(v: i32) -> u32 {
    v as u32 ^ 1 << 31
}

/// The join of the n × n squares at `cell` from `q`, each its whole (its
/// last level); none (`ground::UNBOUNDED`) where one is not held present.
pub fn joined(w: &impl Squares, cell: Zoom, q: [i32; 2], n: u32) -> [i32; 2] {
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

/// Entry `e` of the eye's window at `cell` (`joined_index`), folded off
/// its slots: the block of squares its slots hold, aligned as the slots
/// are (`ACROSS` a multiple of the block's side), keyed by the square in
/// its first slot and joined (`joined`); the zero entry where that slot
/// holds none present, or the block is not held whole, as where it
/// straddles the window's range about its centre.
pub fn fold(w: &impl Squares, cell: Zoom, e: u32) -> [u32; JOINED_WORDS as usize] {
    let mut l = 1;
    while l < JOINED_LEVELS && e >= joined_index(l + 1, [0, 0]) {
        l += 1;
    }
    let (b, across) = (e - joined_index(l, [0, 0]), ACROSS >> l);
    let first = [(b % across) << l, (b / across) << l];
    let (q, here) = w.square(cell, first[1] * ACROSS + first[0]);
    let [lo, hi] = if here { joined(w, cell, q, 1 << l) } else { crate::ground::UNBOUNDED };
    // Component-wise: a GPU has no array equality.
    if lo == crate::ground::UNBOUNDED[0] && hi == crate::ground::UNBOUNDED[1] {
        return [0; JOINED_WORDS as usize];
    }
    [key_word(q[0]), key_word(q[1]), crate::ground::bound_word([lo, hi]) as u32, 0]
}

/// The join an entry `e` holds for the block of squares from `q`
/// (`fold`): its bound where its key is `q`, else none
/// (`ground::UNBOUNDED`), the zero entry and another block's alike.
pub fn unfold(e: [u32; JOINED_WORDS as usize], q: [i32; 2]) -> [i32; 2] {
    if e[0] == key_word(q[0]) && e[1] == key_word(q[1]) {
        crate::ground::bound_of(e[2] as i32)
    } else {
        crate::ground::UNBOUNDED
    }
}

/// The lowest and highest the cells at `cell` reach under the node
/// `(zoom, c)`: off the finest window holding them present, stepped from
/// `place`, which becomes where they are held (`step`), one
/// entry of its square's levels, level 0 the column holding a node no wider
/// than a cell; and that window's zoom. A node wider than a square is the
/// window's fold of its squares' whole (`whole`, `fold`), one entry. None
/// (`ground::UNBOUNDED`) for a node wider than a window, or over a square
/// not held at `cell`, which reads nothing and only splits more; false where
/// no window holds it, a lack.
pub fn bounded(w: &impl Squares, place: &mut Found, cell: Zoom, (zoom, c): (Zoom, [i32; 2])) -> ([i32; 2], Zoom, bool) {
    // The node's first cell at `cell`, or the cell holding it.
    let g = [crate::index_at(zoom, c[0], cell), crate::index_at(zoom, c[1], cell)];
    if cell > zoom + LEVELS {
        return (whole(w, cell, g, cell - zoom), cell, true);
    }
    let (f, i) = step(w, *place, cell, g);
    *place = f;
    let (from, slot) = (f.from(), f.slot());
    if !f.here() {
        return ([0, 0], from, false);
    }
    // rust-gpu has no `saturating_sub`.
    #[allow(clippy::implicit_saturating_sub)]
    let j = if zoom >= from { 0 } else { from - zoom };
    let at = [(i % SQUARE) >> j, (i / SQUARE) >> j];
    (crate::ground::bound_of(w.level(from, slot, j, at)), from, true)
}

/// The join of the squares at `cell` a node `j` levels over a cell covers,
/// from the one holding its first cell `g`: the eye's window's entry for
/// their block (`fold`), one read, where its key is theirs; none
/// (`ground::UNBOUNDED`) past the window's levels, or where the entry is
/// another block's or none, which only splits more. An entry a frame old
/// is its own block's join or names another (the key), so never a wrong
/// bound.
fn whole(w: &impl Squares, cell: Zoom, g: [i32; 2], j: u32) -> [i32; 2] {
    if j > WINDOW_LEVELS {
        return crate::ground::UNBOUNDED;
    }
    let (l, (q, _)) = (j - LEVELS, square_of([wrap_index(cell, g[0]), g[1]]));
    let m = (ACROSS - 1) as i32;
    unfold(w.joined(cell, joined_index(l, [((q[0] & m) as u32) >> l, ((q[1] & m) as u32) >> l])), q)
}
