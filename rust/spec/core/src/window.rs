// pxl-spec-core/window.rs

//! gpu-portable: the ground's cells as they are held, a window a zoom about
//! a center. A ray observes a zoom's cells only within `RADIUS` of them
//! from the eye (pxl-spec space/layer/demand.rs `band`, the top's by its
//! reach), so a window `WINDOW` cells a side about it holds every cell it
//! can observe there. A window is `ACROSS` bricks a side, each brick
//! held in the entry its place names modulo `ACROSS`, so the window follows
//! its center a brick at a time and no held cell moves. Windows at one
//! zoom hold a brick in the same entry, so a brick at a zoom is held once,
//! in a slot of that entry, however many windows want it (`Entry`). A brick
//! is present once its cells are drawn; a reader of one absent reads the
//! coarser window's (`step`).

use crate::area;
use crate::area::Area;
use crate::area::Shard;
use crate::grid;
use crate::ground;
use crate::ground::Bounds;
use crate::ground::Cell;
use crate::ground::{ELE_WORDS, UNKNOWN, VOID};
use crate::lattice;
use crate::lattice::{GROUND, TILE_CELLS};
use crate::way;
use crate::way::Columns;
use crate::way::Segment;
use crate::way::Stations;
use crate::{wrap_idx, Zoom, LEAF_CELL, TOP_ZOOM};

/// A brick's side in cells, as a shift.
pub const BRICK_SHIFT: u32 = 6;
pub const BRICK: u32 = 1 << BRICK_SHIFT;
pub const BRICK_CELLS: u32 = BRICK * BRICK;
/// Bricks a window holds a side, and in all: as many as hold `RADIUS`
/// about a center anywhere in a brick, to a power of two, as `entry` masks
/// by it.
pub const ACROSS: u32 = ((2 * RADIUS + BRICK).div_ceil(BRICK)).next_power_of_two();
pub const ENTRIES: u32 = ACROSS * ACROSS;
/// A window's side in cells.
pub const WINDOW: u32 = ACROSS * BRICK;
/// The eye's coarsest window's cells' zoom: the top's.
pub const FIRST_CELL: Zoom = GROUND.cell_zoom(TOP_ZOOM);
/// The eye's windows, every zoom the pyramid draws, coarsest first, which
/// the GPU's cast reads: the one at `cell` is `cell − FIRST_CELL`.
pub const EYE_WINDOWS: u32 = LEAF_CELL - FIRST_CELL + 1;

/// The cells' zoom of the eye's window `w`, coarsest first: what the layout
/// lays it at and the GPU's decor reads a brick's window at.
pub const fn eye_cell(w: u32) -> Zoom {
    FIRST_CELL + w
}
/// Every window: the eye's; the pivot's, the leaves' and one over, what
/// walking and editing read; a destination's one, at the zoom its line says
/// its ground, found exactly (pxl-kernel domain/layer/terrain/window.rs).
pub const WINDOWS: u32 = EYE_WINDOWS + 3;
/// The slots every window's bricks are held in, `WINDOWS` an entry, slot
/// `k × ENTRIES + s` entry `s`'s `k`-th (pxl-kernel domain/layer/terrain/window.rs
/// `Slot`): no more bricks want an entry than there are windows, so the
/// most the GPU holds, and so the most of their cells read back at once,
/// where no two windows at a zoom overlap. A brick two of them want takes
/// one.
pub const SLOTS: u32 = WINDOWS * ENTRIES;
/// What the GPU's fill costs a frame, measured at dpr 2 on an M-series Mac
/// (2026-10-17, the `fill` span): its passes whatever it is handed, and each
/// brick more; handed one brick a frame it took 2.2 ms, sixteen 5.5 ms.
pub const FILL_MS: f32 = 2.0;
pub const FILL_BRICK_MS: f32 = 0.22;
/// The share of a frame the fill may take.
pub const FILL_SHARE_MS: f32 = 4.0;
/// The bricks a frame hands the fill at most, nearest first (pxl-kernel
/// domain/layer/terrain/window.rs `due`): its share less its passes, over a brick.
pub const HANDED: u32 = ((FILL_SHARE_MS - FILL_MS) / FILL_BRICK_MS) as u32;

/// A brick handed to the GPU's fill, in words: its slot, where it lies in
/// its tile (`place_word`, in bricks), its cells' zoom, its window's as the
/// window holds it (pxl-kernel domain/layer/terrain/window.rs `Window::cell`; a
/// destination's moves with its line), never derived from the index; its
/// first cell at that zoom as the world names it, X wrapped there, then Y,
/// two words; and what is known of the service area at it (`area::Shard`,
/// its word), which the ways' fill lays the area's edge by
/// (`way::Columns::is_walled`).
pub const FRESH_BRICK: u32 = 0;
pub const FRESH_PLACE: u32 = FRESH_BRICK + 1;
pub const FRESH_CELL: u32 = FRESH_PLACE + 1;
pub const FRESH_FIRST: u32 = FRESH_CELL + 1;
pub const FRESH_SHARD: u32 = FRESH_FIRST + 2;
pub const FRESH_WORDS: u32 = FRESH_SHARD + 1;

/// A fresh brick's words (`FRESH_WORDS`), `place` its `place_word`.
pub const fn fresh(
    brick: u32,
    place: u32,
    cell: Zoom,
    first: [i32; 2],
    shard: Shard,
) -> [u32; FRESH_WORDS as usize] {
    let mut w = [0; FRESH_WORDS as usize];
    (
        w[FRESH_BRICK as usize],
        w[FRESH_PLACE as usize],
        w[FRESH_CELL as usize],
    ) = (brick, place, cell);
    (
        w[FRESH_FIRST as usize],
        w[FRESH_FIRST as usize + 1],
        w[FRESH_SHARD as usize],
    ) = (first[0] as u32, first[1] as u32, shard.word());
    w
}

/// Where a brick lies in its tile, in bricks, as one word: X low, Y high.
pub const fn place_word([x, y]: [u32; 2]) -> u32 {
    x | y << 16
}

/// `place_word`'s place.
pub const fn place_of(w: u32) -> [u32; 2] {
    [w & 0xffff, w >> 16]
}

/// The brick a fresh brick's first cell (`FRESH_FIRST`) lies in, at its
/// cells' zoom: what a reader of the handed records finds its entry by.
pub const fn fresh_brick(first: [u32; 2]) -> [i32; 2] {
    [
        first[0] as i32 >> BRICK_SHIFT,
        first[1] as i32 >> BRICK_SHIFT,
    ]
}

/// How far from its center a window holds every cell, in its own cells: a
/// zoom's band ends `CELLS_A_RAD` of its cells from the eye, and the
/// window holds `WINDOW_MARGIN` times that (`grid::SPLIT`).
pub const RADIUS: u32 = grid::SPLIT as u32;

/// The first brick of the window at `cell` about the cell `c`:
/// ⌊(c − RADIUS) / BRICK⌋ a side, X wrapped round the world. Its bricks
/// hold the cells [c − RADIUS, c + RADIUS] wherever in a brick `c` lies.
pub fn origin(cell: Zoom, c: [i32; 2]) -> [i32; 2] {
    let first = |v: i32| (v - RADIUS as i32) >> BRICK_SHIFT;
    [wrap_idx(bricks_zoom(cell), first(c[0])), first(c[1])]
}

/// The zoom a window's bricks are counted at: its cells' over `BRICK_SHIFT`.
pub const fn bricks_zoom(cell: Zoom) -> Zoom {
    cell - BRICK_SHIFT
}

/// The entry brick `q` is held in.
pub fn entry(q: [i32; 2]) -> u32 {
    let m = (ACROSS - 1) as i32;
    ((q[1] & m) as u32) * ACROSS + (q[0] & m) as u32
}

/// The brick holding cell `g`, and where the cell lies in it, X then Y:
/// a reader spreads it to an index (`idx_of`) only where it reads a
/// column, and shifts it to a level's entry as it is.
pub fn brick_of(g: [i32; 2]) -> ([i32; 2], [u32; 2]) {
    let m = (BRICK - 1) as i32;
    (
        [g[0] >> BRICK_SHIFT, g[1] >> BRICK_SHIFT],
        [(g[0] & m) as u32, (g[1] & m) as u32],
    )
}

/// Where a brick's levels begin among them, in entries: level `j`, from 0,
/// each entry the lowest and highest of the 2^j × 2^j cells under it, the
/// ground's and the ways' apart (`ground::Bounds`) — level 0 a cell's own
/// column (`fill::level`).
pub const fn level_at(j: u32) -> u32 {
    (4 * BRICK_CELLS - ((4 * BRICK_CELLS) >> (2 * j))) / 3
}

/// Entry `[x, y]` of level `j` among a brick's levels, each level laid as
/// its cells are (`idx_of`).
pub const fn level_idx(j: u32, at: [u32; 2]) -> u32 {
    level_at(j) + idx_of(at)
}

/// Cell `[x, y]`'s index among a brick's cells (or a level's entries), the
/// one place it is worked out: Morton order, a quadtree's nodes laid as the
/// Z curve visits them, so an entry's four children a level under it are
/// four in a row and the cells about one lie near it, as a texture's tiling
/// lays them for its 2D cache (decision 20261014, the grid's storage by the
/// 2D cache). A cell and the one after it in x, x even, are two in a row.
pub const fn idx_of([x, y]: [u32; 2]) -> u32 {
    spread(x) | spread(y) << 1
}

/// The cell at index `i` (`idx_of`'s inverse).
pub const fn of_idx(i: u32) -> [u32; 2] {
    [gather(i), gather(i >> 1)]
}

/// `v`'s low sixteen bits spread to the even ones: a Morton code's half.
const fn spread(v: u32) -> u32 {
    let v = (v | v << 8) & 0x00ff_00ff;
    let v = (v | v << 4) & 0x0f0f_0f0f;
    let v = (v | v << 2) & 0x3333_3333;
    (v | v << 1) & 0x5555_5555
}

/// `v`'s even bits gathered to the low sixteen: `spread`'s inverse.
const fn gather(v: u32) -> u32 {
    let v = v & 0x5555_5555;
    let v = (v | v >> 1) & 0x3333_3333;
    let v = (v | v >> 2) & 0x0f0f_0f0f;
    let v = (v | v >> 4) & 0x00ff_00ff;
    (v | v >> 8) & 0x0000_ffff
}
const _: () = assert!(BRICK <= 1 << 16, "a brick's side spreads in sixteen bits");
pub const LEVELS: u32 = BRICK_SHIFT;
/// A level entry's words: its bounds' (`ground::Bounds`), each a bound word.
pub const LEVEL_ENTRY_WORDS: u32 = 2;
pub const LEVEL_WORDS: u32 = LEVEL_ENTRY_WORDS * level_at(LEVELS + 1);

/// A tile's ways are cut into bins `WAYS_ACROSS` a side, each
/// `1 << WAYS_SHIFT` cells: a brick lays its lines by them (`LINES_AT`),
/// which its fill keeps its columns' spans off.
pub const WAYS_ACROSS: u32 = 32;
pub const WAYS_SHIFT: u32 = (TILE_CELLS / WAYS_ACROSS).trailing_zeros();
const WAYS_BINS_ACROSS: u32 = BRICK >> WAYS_SHIFT;
pub const WAYS_BINS: u32 = WAYS_BINS_ACROSS * WAYS_BINS_ACROSS;

/// The most entries a brick holds of its tile's ways: one a segment and a
/// bin (`WAYS_SHIFT`) its footprint reaches. The densest brick
/// of the format-12 set, z15 010332102333322 (2026-10-17; z14's most 711);
/// a tile past it fails its bake (pxl-spec bake/tile/terrain.rs `ways_fit`).
pub const WAYS_BRICK_SEGMENTS: u32 = 1065;

/// A brick's lines, laid by the CPU as the brick is filled (pxl-spec
/// bake/tile/terrain.rs `brick_lines`) for its fill to keep its columns'
/// spans off (`fill::spans`): its tile's header (`way::header`), a word a bin
/// (`WAYS_BINS`), its first entry and its count over it; the entries, two a
/// word, each one of the brick's segments, a bin's in their order; then
/// the segments a bin reaches, in their lines' order, `way::SEGMENT_WORDS`
/// each. None holds more than `WAYS_BRICK_SEGMENTS` of either.
pub const LINES_ENTRIES_AT: u32 = way::HEADER_WORDS + WAYS_BINS;
pub const LINES_SEGMENTS_AT: u32 = LINES_ENTRIES_AT + WAYS_BRICK_SEGMENTS.div_ceil(2);
pub const LINES_WORDS: u32 = LINES_SEGMENTS_AT + WAYS_BRICK_SEGMENTS * way::SEGMENT_WORDS as u32;

/// The most runs a square's columns read as (`ways::runs`), a bore's
/// lining among them: the most of the format-12 set (2026-10-18), measured
/// with every tile at every zoom (z15's 15320, tile [11707, 3502] square (7, 6); z14's most
/// 7867); a tile past it fails its bake (pxl-spec bake/tile/terrain.rs
/// `runs_fit`).
pub const RUNS_SQUARE: u32 = 15320;

/// A handed square's scratch, the GPU's own for the frame it is filled
/// (`HANDED` of them, each `ways::tree::scratch_words` of the blocks its
/// tree holds, `ways::tree::SCRATCH_WORDS` at the renderer's cap): its columns'
/// runs, what its ways write along z (`ways::run_word`), lowest first, each
/// to the next's first block and a column's last `NOTHING` on, a column no
/// way writes in none (`RUNS_AT`), each column's first among them, row by
/// row, and the square's count past the last (`FIRSTS_AT`); then its tree's
/// (`ways::tree::FREED_AT`). Its levels' ways' bound and its ways' tree
/// are made off the runs (pxl-spec-core fill.rs `level`, `ways::tree::leaf`),
/// and they go with the frame. Before the tree's, each column's spans, the
/// square's and the ring of columns about it, which a column's runs read
/// beside its own (`SPANS_AT`, fill.rs `spans`).
pub const FIRSTS_AT: u32 = 0;
pub const RUNS_AT: u32 = FIRSTS_AT + SQUARE_CELLS + 1;
/// The columns whose spans a square's runs read: the square's and a ring
/// of one about it, `SPANS_ACROSS` a side, row by row from the ring's
/// first.
pub const SPANS_ACROSS: u32 = SQUARE + 2;
pub const SPANS_COLUMNS: u32 = SPANS_ACROSS * SPANS_ACROSS;
/// A column's spans in words (`way::Spans`): a head word, its count, the
/// count's high bit set where they did not fit `way::SPANS`, and its
/// ground's top in cells over them, an `i16`, `i16::MIN` for none (fill.rs
/// `head_word`);
/// then each span's lowest and past its highest, an `i16` each as the
/// world's z is, held to it (`answer::bound_word`), and its value.
pub const SPANS_COLUMN_WORDS: u32 = 1 + 2 * crate::way::SPANS as u32;
pub const TREE_AT: u32 = SPANS_AT + SPANS_COLUMNS * SPANS_COLUMN_WORDS;

/// A square's columns, a word a cell, row by row: a cell's ground's top in
/// leaf cells, an `i16`, low (`column_top`); and over it which of the four
/// columns beside it touch its top cell at the square's zoom
/// (`column_touches`), the ground's half of the answer's touching faces
/// (`answer`): its top's face below touches by the top's own rule
/// (`ground::top`), and one beside touches where its top reaches this one's
/// top cell (pxl-spec-core fill.rs `level`).
pub const COLUMN_WORDS: u32 = SQUARE_CELLS;

/// A column word's ground top.
pub const fn column_top(w: u32) -> i32 {
    (w << 16) as i32 >> 16
}

/// A column word's columns beside it that touch its top, a bit each: X's
/// lower then higher, Y's lower then higher (`TOUCH_X_LO` …).
pub const fn column_touches(w: u32) -> u32 {
    (w >> 16) & 0xf
}

pub const TOUCH_X_LO: u32 = 1;
pub const TOUCH_X_HI: u32 = 2;
pub const TOUCH_Y_LO: u32 = 4;
pub const TOUCH_Y_HI: u32 = 8;

/// A square's cells as its fill lays them, two a word, cell `i` in the
/// `i % 2`-th half (`laid_at`): its own look, low (`ground::own` of its four
/// at the brick's zoom), and which of the eight leaf cells about it are
/// past the area, high (`area::gone_ring`, none at a brick coarser than a
/// leaf's): what a read of it lays the area's edge by (`area::laid`),
/// never working out its majority or its ring again.
pub const LAID_WORDS: u32 = BRICK_CELLS / 2;

/// Cell `i`'s half of a laid word `w`.
pub const fn laid_at(w: u32, i: u32) -> u32 {
    (w >> (16 * (i % 2))) & 0xffff
}

/// A cell's laid half: its own look and the leaf cells about it gone.
pub const fn laid_half(look: u32, gone: u32) -> u32 {
    look & 0xff | gone << 8
}

/// A brick's words on a GPU, what outlives its fill, its lanes in order:
/// its columns, its levels, its cells as laid and its ways' tree's root. The
/// windows' bricks are one storage buffer bound whole, under WebGPU's
/// floor (pxl-render gpu/ray/ground.rs `GROUND_BYTES`).
pub const COLUMNS_AT: u32 = 0;
pub const LEVELS_AT: u32 = COLUMNS_AT + COLUMN_WORDS;
pub const LAID_AT: u32 = LEVELS_AT + LEVEL_WORDS;
/// Its ways' tree's root word: the family of its own nodes along z in the
/// pool and the first one's Z (`way::tree::root_word`); 0, family 0, never
/// handed out, where it has none.
pub const ROOT_AT: u32 = LAID_AT + LAID_WORDS;
pub const BRICK_WORDS: u32 = ROOT_AT + 1;
/// What comes back of a brick: all of it, so the CPU reads a column as the
/// GPU does, never working it out again.
pub const BACK_WORDS: u32 = BRICK_WORDS;

/// A brick handed to the fill as a frame's fresh list holds it, a record
/// each, `HANDED` at most: its words (`fresh`), then what only its fill
/// reads, gone with the frame — its four looks a cell, its tile's padded
/// samples, two a word, and its lines.
pub const LOOKS_AT: u32 = FRESH_WORDS;
pub const ELE_AT: u32 = LOOKS_AT + BRICK_CELLS;
pub const LINES_AT: u32 = ELE_AT + ELE_WORDS;
pub const HANDED_WORDS: u32 = LINES_AT + LINES_WORDS;

/// Where the `k`-th brick a frame hands begins in its fresh list.
pub const fn handed_at(k: u32) -> u32 {
    k * HANDED_WORDS
}

/// The windows as one side holds them: per window, a zoom's cells about
/// its center, which brick each entry holds once present, and the slots
/// their cells are held in.
pub trait Bricks {
    /// The slot brick `q` at `cell` is held present in, which the reads
    /// below take (`Entry::slot`); false where no window holds it.
    fn present(&self, cell: Zoom, q: [i32; 2]) -> (u32, bool);
    /// The brick the eye's window at `cell` wants in `entry`, and whether
    /// present: `present` the other way.
    fn brick(&self, cell: Zoom, entry: u32) -> ([i32; 2], bool);
    /// The four looks of cell `i` of the brick in `slot` (`ground::looks`).
    fn look(&self, cell: Zoom, slot: u32, i: u32) -> u32;
    /// Column word `i` of the brick in `slot` (`COLUMN_WORDS`), `i` to
    /// `BRICK_CELLS`.
    fn column(&self, cell: Zoom, slot: u32, i: u32) -> u32;
    /// Entry `at` of level `j` of the brick in `slot`, its bounds
    /// (`ground::Bounds`) as bound words (`answer::bound_word`): its
    /// columns' joined (`join`), level 0 one.
    fn level(&self, cell: Zoom, slot: u32, j: u32, at: [u32; 2]) -> [i32; 2];
    /// Cell `i` as its fill laid it (`LAID_WORDS`, `laid_at`).
    fn laid(&self, cell: Zoom, berth: u32, i: u32) -> u32;
    /// Entry `e` of the eye's window at `cell`'s levels over its squares
    /// (`JOINED_WORDS`, `fold`), as this side folded them over the squares
    /// it holds; the zero entry, none, where it holds no fold.
    fn joined(&self, cell: Zoom, e: u32) -> [u32; JOINED_WORDS as usize];
    /// The eye's window at `cell`'s first brick, which its blocks of
    /// bricks are counted from (`fold`), and whether it is laid out
    /// (`ORIGIN_AT`).
    fn origin(&self, cell: Zoom) -> ([i32; 2], bool);
    /// The root of the brick in `slot`'s ways' tree (`ROOT_AT`), 0 for none.
    fn root(&self, cell: Zoom, slot: u32) -> u32;
}

/// One brick as its fill reads it, for the ways' one rule
/// (`way::Columns`): its cells' zoom, where it lies in its tile (in
/// bricks), word `k` of its lines (`LINES_AT`), cell `i` as `heights` laid
/// it (`LAID_WORDS`), its own look and the area's edge it is walled by, and
/// column `c`'s height off its tile's samples, `c`
/// counted from its first cell, the ring of columns about it included.
/// Its columns are its cells and that ring, a ring column read off the
/// edge bin it borders, whose lines hold every one that reaches it
/// (`way::footprint`); one past the ring has no segment.
pub trait Brick {
    fn cell(&self) -> Zoom;
    fn place(&self) -> [u32; 2];
    fn line(&self, k: u32) -> u32;
    /// Cell `i` as its fill laid it (`LAID_WORDS`), before its ways: what
    /// they wall it by.
    fn laid(&self, i: u32) -> u32;
    fn height(&self, c: [i32; 2]) -> i32;
    /// Column `c`'s spans and whether they fit, where the square keeps them
    /// worked out already (`SPANS_AT`, pxl-spec-core fill.rs `spans`):
    /// whether it does, and they. None by default, each worked out off its
    /// lines (`ways::column`).
    fn kept_spans(&self, _c: [i32; 2]) -> (bool, crate::ways::Spans, bool) {
        (false, crate::ways::Spans::NONE, true)
    }
}

/// Whether column `c` is the brick's or its ring's.
fn is_held(c: [i32; 2]) -> bool {
    c[0] >= -1 && c[1] >= -1 && c[0] <= BRICK as i32 && c[1] <= BRICK as i32
}

/// Column `c`'s cell index in the brick, none outside it.
fn cell_idx(c: [i32; 2]) -> u32 {
    if c[0] < 0 || c[1] < 0 || c[0] >= BRICK as i32 || c[1] >= BRICK as i32 {
        return BRICK_CELLS;
    }
    idx_of([c[0] as u32, c[1] as u32])
}

/// Column `c`'s bin word, a ring column's the edge bin it borders: its
/// first entry and its count; none past the ring.
fn bin_of(sq: &impl Brick, c: [i32; 2]) -> (u32, u32) {
    if !is_held(c) {
        return (0, 0);
    }
    let (x, y) = (
        c[0].clamp(0, BRICK as i32 - 1) as u32,
        c[1].clamp(0, BRICK as i32 - 1) as u32,
    );
    let b = (y >> WAYS_SHIFT) * WAYS_BINS_ACROSS + (x >> WAYS_SHIFT);
    let w = sq.line(way::HEADER_WORDS + b);
    (w & 0xffff, w >> 16)
}

/// Entry `e` of the brick's lines: the place of a segment among its.
fn entry_of(sq: &impl Brick, e: u32) -> u32 {
    (sq.line(LINES_ENTRIES_AT + e / 2) >> (16 * (e % 2))) & 0xffff
}

fn segment_at(sq: &impl Brick, at: u32) -> Segment {
    let base = LINES_SEGMENTS_AT + at * way::SEGMENT_WORDS as u32;
    let mut w = [0; way::SEGMENT_WORDS];
    let mut k = 0;
    while k < w.len() {
        w[k] = sq.line(base + k as u32);
        k += 1;
    }
    Segment::of(w)
}

fn header_of(sq: &impl Brick) -> (f32, Stations) {
    let mut w = [0; way::HEADER_WORDS as usize];
    let mut k = 0;
    while k < w.len() {
        w[k] = sq.line(k as u32);
        k += 1;
    }
    way::of_header(w)
}

/// A brick's columns as its lines and its cells give them.
impl<Q: Brick> Columns for Q {
    fn cell(&self) -> Zoom {
        Brick::cell(self)
    }
    fn per_m(&self) -> f32 {
        header_of(self).0
    }
    fn cnt(&self, c: [i32; 2]) -> u32 {
        bin_of(self, c).1
    }
    fn segment(&self, c: [i32; 2], k: u32) -> Segment {
        segment_at(self, entry_of(self, bin_of(self, c).0 + k))
    }
    /// In its tile's cells: the brick's first, then the cell's middle.
    fn center(&self, c: [i32; 2]) -> [f32; 2] {
        let p = self.place();
        let center = lattice::CELL_CENTER;
        [
            (p[0] * BRICK) as f32 + c[0] as f32 + center,
            (p[1] * BRICK) as f32 + c[1] as f32 + center,
        ]
    }
    /// Off its cell as laid (`area::laid`), as a read of it walls it: a
    /// brick's first cell lies on a leaf cell's first, so `c` places the
    /// column in its leaf cell as its world index does. A ring column is
    /// none of the brick's, never walled.
    fn is_walled(&self, c: [i32; 2]) -> bool {
        let i = cell_idx(c);
        i != BRICK_CELLS && area::laid(self.laid(i), Brick::cell(self), c) == UNKNOWN
    }
    fn stations(&self) -> Stations {
        header_of(self).1
    }
    /// Its own look, before the service area's wall is laid, as its fill
    /// laid it (`LAID_WORDS`): the wall is the descent's to lay (`ground`'s
    /// reader).
    fn look(&self, c: [i32; 2]) -> u32 {
        let i = cell_idx(c);
        if i == BRICK_CELLS {
            VOID
        } else {
            self.laid(i) & 0xff
        }
    }
    fn top(&self, c: [i32; 2]) -> i32 {
        if !held(c) {
            return 0;
        }
        crate::ground::cells(Square::cell(self), self.height(c) as f32)
    }
    fn spans(&self, c: [i32; 2]) -> (crate::ways::Spans, bool) {
        let (kept, spans, fits) = self.kept_spans(c);
        if kept {
            (spans, fits)
        } else {
            crate::ways::column(self, c)
        }
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

/// Where a node's cells at `cell`, a brick's or fewer, are held, as `step`
/// finds them: the zoom they were asked at, the window holding them present
/// (its zoom `from`, at `cell` or coarser, and the slot), or none; one word,
/// as a descent carries it down beside what is placed (`overwrite::Held`).
/// `step` reads only the brick at `cell` holding them and the bricks over
/// it, so every node whose cells lie in that brick has this answer: a
/// descent hands it from a node to the nodes under it (`step`). `NONE` was
/// asked at no zoom.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Found(u32);

/// A slot, which a GPU and the CPU name alike (`SLOTS`).
const FOUND_SLOT_BITS: u32 = SLOTS.next_power_of_two().trailing_zeros();
const FOUND_HERE: u32 = 1 << FOUND_SLOT_BITS;
const FOUND_FROM_SHIFT: u32 = FOUND_SLOT_BITS + 1;
const FOUND_CELL_SHIFT: u32 = FOUND_FROM_SHIFT + crate::ZOOM_BITS;
const FOUND_ZOOM_MASK: u32 = (1 << crate::ZOOM_BITS) - 1;
/// A `Found`'s word's bits: its slot, here, and its two zooms.
const FOUND_BITS: u32 = FOUND_CELL_SHIFT + crate::ZOOM_BITS;
const _: () = assert!(FOUND_BITS <= 32);
const _: () = assert!(
    FIRST_CELL >= 2,
    "a zoom asked is never NONE's, nor one finer"
);

impl Found {
    pub const NONE: Found = Found(0);

    const fn of(cell: Zoom, from: Zoom, slot: u32, here: bool) -> Found {
        Found(
            cell << FOUND_CELL_SHIFT
                | from << FOUND_FROM_SHIFT
                | (here as u32) << FOUND_SLOT_BITS
                | slot,
        )
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

/// The cell at `from` that column `c` of the cells at `zoom` lies in.
fn cell_at(zoom: Zoom, from: Zoom, c: [i32; 2]) -> [i32; 2] {
    let s = zoom - from;
    [wrap_idx(from, c[0] >> s), c[1] >> s]
}

/// The window holding a node's cells at `cell` from its first, `g`, stepped
/// from `above`, what a node over it found, or `NONE`, reading only what
/// `above` leaves unknown. Only a node a brick wide or less sets a place
/// (`bounded`), so every node a descent asks under it, a sibling from its
/// parent's stack entry too, lies in that node, and at each zoom to
/// `above`'s cell in the brick that held `above`'s cells there. The
/// windows are asked finest first, each a zoom's, and the first holding the
/// cell present answers, so `above` says what each of them answers: none
/// present over `above`'s window, it present. Asked at `above`'s cell or
/// coarser down to its window, the answer is that window, read nowhere.
/// Asked finer, only the zooms finer than `above`'s are unknown: they are
/// looked at finest first, then `above`'s answer stands, a look a zoom of
/// the step. With nothing above, or coarser than `above`'s window, every
/// window from `cell`'s to the first (`FIRST_CELL`) is looked at, and none
/// holding it is a lack.
pub fn step(w: &impl Bricks, above: Found, cell: Zoom, g: [i32; 2]) -> (Found, [u32; 2]) {
    // The zooms `above` leaves unknown, finest first, down to the one it
    // answers, or to the first window where it answers none.
    let known = above != Found::NONE && cell >= above.from();
    let (floor, mut f) = if known {
        (
            above.cell(),
            Found::of(cell, above.from(), above.slot(), above.here()),
        )
    } else {
        (FIRST_CELL - 1, Found::of(cell, FIRST_CELL, 0, false))
    };
    let mut c = cell;
    while c > floor {
        let (slot, here) = w.present(c, brick_of(cell_at(cell, c, g)).0);
        if here {
            f = Found::of(cell, c, slot, true);
            break;
        }
        c -= 1;
    }
    (f, brick_of(cell_at(cell, f.from(), g)).1)
}

/// The zoom a cell at `cell` is read at, the eye's windows' (`FIRST_CELL`
/// to `LEAF_CELL`): past the leaves a leaf's, coarser than the coarsest
/// window that window's.
pub const fn held_to(cell: Zoom) -> Zoom {
    if cell > LEAF_CELL {
        LEAF_CELL
    } else if cell < FIRST_CELL {
        FIRST_CELL
    } else {
        cell
    }
}

/// The ground's cell `g` at `cell`, off the finest window holding it
/// present, stepped from `place` (`step`): its four looks, its column's
/// word (its height), it as laid, and where it is held; `void` at sea level
/// where none holds it.
pub fn cell(w: &impl Bricks, place: Found, cell: Zoom, g: [i32; 2]) -> (u32, u32, u32, Found) {
    let (f, at) = step(w, place, cell, g);
    if !f.here() {
        return (VOID, 0, laid_half(VOID, 0), f);
    }
    let (from, slot, i) = (f.from(), f.slot(), idx_of(at));
    (
        w.look(from, slot, i),
        w.column(from, slot, i),
        w.laid(from, slot, i),
        f,
    )
}

/// Column `b` at `zoom` as a reader reads it: the cell drawing it, past
/// the leaves a leaf's, stepped from `place`, where the column's own cells
/// were found (`step`), with the area's edge laid on it off the cell as its
/// fill laid it (`area::laid`), and the columns beside it touching its top
/// where it is read at the zoom of the square holding it, none elsewhere;
/// and where it is held.
pub fn ground(
    w: &(impl Squares + crate::area::Area),
    place: Found,
    zoom: Zoom,
    b: [i32; 2],
) -> (crate::ground::Cell, Found) {
    let read = held_to(zoom);
    let (word, column, laid, f) = cell(w, place, read, cell_at(zoom, read, b));
    let (top, part) = ground::top(zoom, column_top(column) as f32);
    let cell = Cell {
        look: area::laid(laid, zoom, b),
        top,
        part,
        fold: crate::ground::looks(word),
        touch: if f.here() && f.from() == zoom {
            column_touches(column)
        } else {
            0
        },
    };
    (cell, f)
}

/// Entry `at` of level `j` of a brick, from 1, over the level under it,
/// written before: the join of the four under it, each source's its own
/// (`ground::joined`), as bound words.
pub fn join(w: &impl Bricks, cell: Zoom, slot: u32, j: u32, [x, y]: [u32; 2]) -> [i32; 2] {
    let under =
        |q: u32| ground::bounds_of(w.level(cell, slot, j - 1, [2 * x + q % 2, 2 * y + q / 2]));
    let mut b = under(0);
    let mut q = 1;
    while q < 4 {
        b = ground::joined(b, under(q));
        q += 1;
    }
    ground::bounds_word(b)
}

/// Levels a window bounds a node over, its bricks' own and then theirs
/// joined, to the window whole.
pub const WINDOW_LEVELS: u32 = LEVELS + ACROSS.trailing_zeros();

/// The levels over a window's bricks, from one over a brick
/// (`LEVELS + 1`) to the window whole: a node wider than a brick bounded
/// by the join of the bricks it covers, folded where the windows' entries
/// are written (`fold`) and read as one entry (`bounded`), not joined
/// again at every node that asks.
pub const JOINED_LEVELS: u32 = WINDOW_LEVELS - LEVELS;
/// A window's entries: level `l` over a brick has `(ACROSS >> l)²`, an
/// aligned block of `2^l` bricks a side each.
pub const JOINED_ENTRIES: u32 = joined_idx(JOINED_LEVELS + 1, [0, 0]);
/// An entry's words: its key, the block's first brick at its cells' zoom,
/// X then Y, each with its sign flipped (`key_word`), so the zero entry,
/// as fresh memory holds it, names no brick; then the join's bounds, the
/// ground's and the ways' (`ground::bounds_word`): a row of the windows'
/// buffer, an `IVec4` as their entry table's are, after it (`JOINED_AT`).
pub const JOINED_WORDS: u32 = 4;
/// A row of the windows' table, as the kernel writes it (pxl-kernel
/// domain/layer/terrain/window.rs `table`) and the GPU reads it (pxl-shader
/// `World`), an `IVec4` of its buffer: the brick an entry's window wants
/// there; where its cells are present, the slot holding them plus one
/// (`slot`), which another window at its zoom may have filled, 0 while
/// they are not; and the fill they are on their way from, 0 where none is.
/// Neither: wanted, its tile not yet handed it.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Entry {
    pub brick: [i32; 2],
    pub present: i32,
    pub ticket: u32,
}

const _: () = assert!(size_of::<Entry>() == 16, "an entry is a row of four words");

impl Entry {
    pub const fn absent(brick: [i32; 2]) -> Entry {
        Entry {
            brick,
            present: 0,
            ticket: 0,
        }
    }

    /// The row the buffer's four words hold.
    pub fn of([x, y, z, w]: [i32; 4]) -> Entry {
        Entry {
            brick: [x, y],
            present: z,
            ticket: w as u32,
        }
    }

    /// Its brick, and whether present (`Bricks::brick`).
    pub fn held(&self) -> ([i32; 2], bool) {
        (self.brick, self.present != 0)
    }

    /// Whether it holds `q` present (`Bricks::present`).
    pub fn holds(&self, q: [i32; 2]) -> bool {
        self.present != 0 && self.brick[0] == q[0] && self.brick[1] == q[1]
    }

    /// The slot its brick's cells are held in, while present.
    pub fn slot(&self) -> u32 {
        (self.present - 1) as u32
    }
}

/// The row of the windows' buffer each window's first brick is at: past
/// every window's entries, a row a window shaped as an entry's (`Entry`), its
/// brick the window's first and `present` 1 while it is laid out
/// (`Bricks::origin`).
pub const ORIGIN_AT: u32 = SLOTS;
/// The row of the windows' buffer the eye's windows' entries begin at:
/// past their entries and their first bricks.
pub const JOINED_AT: u32 = ORIGIN_AT + WINDOWS;
/// The eye's windows' entries, as the GPU lays them: window `cell −
/// FIRST_CELL` a run of `JOINED_ENTRIES`.
pub const EYE_JOINED_WORDS: u32 = JOINED_ENTRIES * JOINED_WORDS * EYE_WINDOWS;

/// Entry of level `l` (from 1) over a brick, its block `at` in blocks of
/// that level, among a window's: the levels' entries in order, a level's
/// in Morton order (`idx_of`), so an entry's four halves are the four
/// entries `4b..4b + 4` of the level under it. The levels under `l` hold
/// `(ACROSS >> m)²` each, `m` from 1: a geometric sum, `(ACROSS² −
/// (ACROSS >> (l − 1))²) / 3`, whole as a difference of two powers of four.
pub const fn joined_idx(l: u32, at: [u32; 2]) -> u32 {
    let under = ACROSS >> (l - 1);
    (ACROSS * ACROSS - under * under) / 3 + idx_of(at)
}
const _: () = assert!(ACROSS.is_power_of_two() && JOINED_LEVELS < ACROSS.trailing_zeros() + 1);

/// A key's word: `v` with its sign flipped, so no brick's is 0.
const fn key_word(v: i32) -> u32 {
    v as u32 ^ 1 << 31
}

/// Entry `b` of level `l` over a brick, its Morton index there
/// (`joined_idx`), of the eye's window at `cell`, folded off the level
/// under it: what a ray reads at `cell` in the block of `2^l` entries a side
/// at `b`, aligned as the entries are (`ACROSS` a multiple of the block's
/// side), the join of its four halves' joins, each read once where the
/// level under it laid it (`Squares::joined`; at level 1 a square's own,
/// `held_square`); keyed by the window's first square (`Squares::origin`),
/// so an entry of a window since moved names another. A block in the
/// window's range is a node's squares; one across its seam, where the
/// slots wrap, holds what the window holds of the two nodes either side,
/// which bounds either from over. The zero entry where the window is not
/// laid out or no window holds a square of the block, a lack. A level is
/// folded once the one under it is.
pub fn fold(w: &impl Squares, cell: Zoom, l: u32, b: u32) -> [u32; JOINED_WORDS as usize] {
    let [x, y] = of_index(b);
    let (first, laid) = w.origin(cell);
    let key = [key_word(first[0]), key_word(first[1])];
    let half = 1 << (l - 1);
    let mut joined = [ground::EMPTY; 2];
    let mut k = 0;
    while laid && k < 4 {
        let (dx, dy) = ((k & 1) * half, (k >> 1) * half);
        let got = if l == 1 {
            let (q, _) = w.square(cell, slot([((x << l) + dx) as i32, ((y << l) + dy) as i32]));
            held_square(w, cell, q)
        } else {
            unfold(w.joined(cell, joined_idx(l - 1, [0, 0]) + 4 * b + k), first)
        };
        // Component-wise: a GPU has no array equality. A half no window
        // holds is none of either source.
        if got[0][0] == crate::ground::UNBOUNDED[0] && got[0][1] == crate::ground::UNBOUNDED[1] {
            return [0; JOINED_WORDS as usize];
        }
        joined = ground::joined(joined, got);
        k += 1;
    }
    if !laid {
        return [0; JOINED_WORDS as usize];
    }
    let [g, ways] = ground::bounds_word(joined);
    [key[0], key[1], g as u32, ways as u32]
}

/// What a ray reads of square `q` at `cell`: its cells off the finest
/// window holding them present, this one or a coarser one, as a node a
/// square wide reads them (`bounded`), so a square this window has yet to
/// hold is bounded by the cells read in its place; none
/// (`ground::UNBOUNDED`) where no window holds it, a lack. Only the
/// squares' own levels are read, never another window's entries, so the
/// windows fold in any order.
fn held_square(w: &impl Squares, cell: Zoom, q: [i32; 2]) -> crate::ground::Bounds {
    let mut place = Found::NONE;
    let (b, _, here, _) = bounded(w, &mut place, cell, (cell - LEVELS, q));
    if here {
        b
    } else {
        [crate::ground::UNBOUNDED; 2]
    }
}
const _: () = assert!(
    FIRST_CELL >= LEVELS,
    "a square is a node at its cells' zoom less `LEVELS`"
);

/// The joins an entry `e` holds of a window whose first square is `q`
/// (`fold`): its bounds where its key is `q`, else none
/// (`ground::UNBOUNDED`), the zero entry and a moved window's alike.
fn unfold(e: [u32; JOINED_WORDS as usize], q: [i32; 2]) -> Bounds {
    if e[0] == key_word(q[0]) && e[1] == key_word(q[1]) {
        ground::bounds_of([e[2] as i32, e[3] as i32])
    } else {
        [ground::UNBOUNDED; 2]
    }
}

/// What the eye's window at `cell` holds of a node wider than a brick,
/// its first cell there `g` and `2^l` bricks a side, `l` from 1: one
/// entry, its entry block's (`fold`), the node's bricks exactly where it
/// lies in the window's range and a superset where it crosses the seam; a
/// node wider than the window the window whole, the top block. And whether
/// no other window need be read: the node lies in the window, or the entry
/// is none (`ground::UNBOUNDED`), a square of its block held by no window,
/// so nothing bounds the node, which only splits more. What the window
/// does not reach of the node a coarser window holds, as a cell's
/// (`step`). The one reader of every entry.
fn held_of(w: &impl Squares, cell: Zoom, g: [i32; 2], l: u32) -> (crate::ground::Bounds, bool) {
    let (first, laid) = w.origin(cell);
    if !laid {
        return ([ground::EMPTY; 2], false);
    }
    let (q, _) = brick_of([wrap_idx(cell, g[0]), g[1]]);
    // The node's first brick from the window's, X the short way round.
    let d = [
        wrap_idx(bricks_zoom(cell), q[0] - first[0]),
        q[1] - first[1],
    ];
    let (side, across) = (1i32 << l, ACROSS as i32);
    if d[0] >= across || d[1] >= across || d[0] + side <= 0 || d[1] + side <= 0 {
        return ([ground::EMPTY; 2], false);
    }
    let all = l <= JOINED_LEVELS
        && d[0] >= 0
        && d[1] >= 0
        && d[0] + side <= across
        && d[1] + side <= across;
    let (level, m) = (
        if l > JOINED_LEVELS { JOINED_LEVELS } else { l },
        (ACROSS - 1) as i32,
    );
    let at = [((q[0] & m) as u32) >> level, ((q[1] & m) as u32) >> level];
    let b = unfold(w.joined(cell, joined_index(level, at)), first);
    // A square no window holds: no coarser window bounds it either.
    if b[0][0] == crate::ground::UNBOUNDED[0] && b[0][1] == crate::ground::UNBOUNDED[1] {
        return (b, true);
    }
    (b, all)
}

/// What the windows hold of the node `(zoom, c)` at the cells from
/// `coarsest` to `finest`, each held to the windows' zooms (`held_to`):
/// the window at each read once, finest first, `finest`'s stepped from
/// `place`, which becomes where its cells are held, a read off a coarser
/// window standing for every zoom down to it; and on under
/// `coarsest`, while no window read there holds all of the node, a cell's
/// own order (`step`), each zoom's cells being its own tile's, which no
/// other's bound holds. The lowest and highest joined, and the finest's
/// window's zoom; none (`ground::UNBOUNDED`) where nothing bounds it, which
/// only splits more; false where a window it needs holds none of it, a
/// lack. The one way a bound over a node is read.
pub fn bound_over(
    w: &impl Bricks,
    place: &mut Found,
    [coarsest, finest]: [Zoom; 2],
    (zoom, c): (Zoom, [i32; 2]),
) -> (Bounds, Zoom, bool) {
    let (top, bottom) = (held_to(finest), held_to(coarsest));
    let (mut b, mut first) = ([ground::EMPTY; 2], top);
    let mut z = top;
    loop {
        let mut at = *place;
        let (got, from, here, all) = bounded(w, &mut at, z, (zoom, c));
        if z == top {
            (*place, first) = (at, from);
        }
        if !here {
            return ([[0, 0]; 2], first, false);
        }
        b = ground::joined(b, got);
        // A read off a coarser window than asked is every zoom's down to it:
        // none between holds the node's brick, so each would read it again.
        let open = b[0][0] <= ground::UNBOUNDED[0] && b[0][1] >= ground::UNBOUNDED[1];
        if open || from <= bottom && all || from == FIRST_CELL {
            return (
                if from <= bottom && all || open {
                    b
                } else {
                    [ground::UNBOUNDED; 2]
                },
                first,
                true,
            );
        }
        z = from - 1;
    }
}

/// The lowest and highest the cells at `cell` reach under the node
/// `(zoom, c)`: off the finest window holding them present, stepped from
/// `place`, which becomes where they are held (`step`), one entry of its
/// brick's levels, level 0 the column holding a node no wider than a
/// cell, and that window's zoom; a node wider than a brick, what the
/// window at `cell` holds of it (`held_of`), and whether it holds all of it
/// (`bound_over` reads on where it does not). False where no window holds
/// it, a lack.
fn bounded(
    w: &impl Bricks,
    place: &mut Found,
    cell: Zoom,
    (zoom, c): (Zoom, [i32; 2]),
) -> (Bounds, Zoom, bool, bool) {
    // The node's first cell at `cell`, or the cell holding it.
    let g = [
        crate::idx_at(zoom, c[0], cell),
        crate::idx_at(zoom, c[1], cell),
    ];
    if cell > zoom + LEVELS {
        let (b, all) = held_of(w, cell, g, cell - zoom - LEVELS);
        return (b, cell, true, all);
    }
    let (f, p) = step(w, *place, cell, g);
    *place = f;
    let (from, slot) = (f.from(), f.slot());
    if !f.here() {
        return ([[0, 0]; 2], from, false, false);
    }
    let (j, at) = entry_at(zoom, from, p);
    (
        ground::bounds_of(w.level(from, slot, j, at)),
        from,
        true,
        true,
    )
}

/// The level of a brick at `from` bounding a node at `zoom`, level 0 a
/// column's own for a node no wider than a cell, and its entry there for
/// the node's first cell, at `[x, y]` in its brick.
fn entry_at(zoom: Zoom, from: Zoom, [x, y]: [u32; 2]) -> (u32, [u32; 2]) {
    #[expect(
        clippy::implicit_saturating_sub,
        reason = "rust-gpu has no `saturating_sub`"
    )]
    let j = if zoom >= from { 0 } else { from - zoom };
    (j, [x >> j, y >> j])
}
