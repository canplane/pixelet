// pxl-engine/clipmap.rs

//! gpu-portable: the ground's cells as they are held, one clipmap level (Tanner 1998) a zoom,
//! around a center. A ray observes a zoom's cells only within `RADIUS` of those cells from
//! the eye (pxl-spec space/layer/demand.rs `band`; the top zoom's by its reach), so a clipmap
//! level `CLIP_SIZE` cells a side around the eye holds every cell it can observe at that
//! zoom. A clipmap level is `ACROSS` bricks a side, and each brick is held in the entry its
//! place names modulo `ACROSS`, so the level follows its center a brick at a time and no held
//! cell moves. Clipmap levels at one zoom hold a brick in the same entry, so a brick at a
//! zoom is held once, in a slot of that entry, however many clipmap levels want it (`Entry`).
//! A brick is present once its cells are drawn; a reader of an absent brick reads the
//! coarser clipmap level's (`step`).

use crate::area;
use crate::area::Area;
use crate::area::Shard;
use crate::grid;
use crate::ground;
use crate::ground::Cell;
use crate::ground::Extent;
use crate::ground::{UNKNOWN, VOID};
use crate::lattice;
use crate::lattice::{GROUND, TILE_CELLS};
use crate::way;
use crate::way::ColumnGrid;
use crate::way::Segment;
use crate::way::StationGrid;
use crate::{wrap_idx, Zoom, LEAF_CELL_ZOOM, MIN_ZOOM};

/// The base-2 logarithm of a brick's side in cells.
pub const BRICK_SHIFT: u32 = 6;
pub const BRICK: u32 = 1 << BRICK_SHIFT;
pub const BRICK_CELLS: u32 = BRICK * BRICK;
/// The bricks a clip level holds a side (`ACROSS`) and in all (`ENTRIES`): enough to hold
/// `RADIUS` around a center anywhere in a brick, rounded up to a power of two, since `entry`
/// masks by it.
pub const ACROSS: u32 = ((2 * RADIUS + BRICK).div_ceil(BRICK)).next_power_of_two();
pub const ENTRIES: u32 = ACROSS * ACROSS;
/// A clip level's side, counted in cells.
pub const CLIP_SIZE: u32 = ACROSS * BRICK;
/// The eye's coarsest clip level's cells' zoom: the top's.
pub const FIRST_CELL_ZOOM: Zoom = GROUND.cell_zoom(MIN_ZOOM);
/// The eye's clip levels, one for every zoom the pyramid draws, coarsest first, which the
/// GPU's cast reads; the one at `cell_zoom` is number `cell_zoom − FIRST_CELL_ZOOM`.
pub const EYE_CLIP_LEVELS: u32 = LEAF_CELL_ZOOM - FIRST_CELL_ZOOM + 1;

/// The cells' zoom of the eye's clip level `w`, counting coarsest first: the zoom the layout
/// lays the level at, and the zoom at which the GPU's decor reads a brick's clip level.
pub const fn eye_cell_zoom(w: u32) -> Zoom {
    FIRST_CELL_ZOOM + w
}
/// Every clip level: the eye's; the pivot's two, at the leaves' zoom and the one above, which
/// walking and editing read; and one for a destination, at the zoom its line reads the ground
/// at, found exactly (pxl-core domain/layer/terrain/clipmap.rs).
pub const CLIP_LEVELS: u32 = EYE_CLIP_LEVELS + 3;
/// The slots every clip level's bricks are held in, `CLIP_LEVELS` to an entry, slot
/// `k × ENTRIES + s` being entry `s`'s `k`-th (pxl-core domain/layer/terrain/clipmap.rs
/// `Slot`). No more bricks want an entry than there are clip levels, so this is the most held
/// at once, reached where no two clip levels at a zoom overlap. A brick that two of them want
/// takes one slot.
pub const SLOTS: u32 = CLIP_LEVELS * ENTRIES;
/// The most bricks a frame hands the GPU once the core's fill has filled them (pxl-core
/// domain/layer/terrain/clipmap.rs `upload`). Each brick is a box of decor's generation
/// (pxl-content `generation::RECT_CAP`), and bricks past this many are handed over in later
/// frames.
pub const FRAME_BRICKS: u32 = 9;

/// How far from its center a clip level holds every cell, in its own cells. A zoom's band
/// ends `CELLS_A_RAD` of its cells from the eye, and the clip level holds `CLIP_MARGIN` times
/// that (`grid::CLIP_CELLS_A_RAD`).
pub const RADIUS: u32 = grid::CLIP_CELLS_A_RAD as u32;

/// The first brick of the clip level at `cell_zoom` around the cell `c`: ⌊(c − RADIUS) /
/// BRICK⌋ on each axis, with X wrapped around the world. Its bricks hold the cells
/// [c − RADIUS, c + RADIUS] wherever in a brick `c` lies.
pub fn origin(cell_zoom: Zoom, c: [i32; 2]) -> [i32; 2] {
    let first = |v: i32| (v - RADIUS as i32) >> BRICK_SHIFT;
    [wrap_idx(bricks_zoom(cell_zoom), first(c[0])), first(c[1])]
}

/// The zoom at which a clip level's bricks are counted: `BRICK_SHIFT` zooms coarser than its
/// cells.
pub const fn bricks_zoom(cell_zoom: Zoom) -> Zoom {
    cell_zoom - BRICK_SHIFT
}

/// The entry brick `q` is held in.
pub fn entry(q: [i32; 2]) -> u32 {
    let m = (ACROSS - 1) as i32;
    ((q[1] & m) as u32) * ACROSS + (q[0] & m) as u32
}

/// The brick holding cell `g`, and where the cell lies in it, X then Y. A reader spreads the
/// place into an index (`idx_of`) only where it reads a column, and shifts it as it is to
/// find a level's entry.
pub fn brick_of(g: [i32; 2]) -> ([i32; 2], [u32; 2]) {
    let m = (BRICK - 1) as i32;
    (
        [g[0] >> BRICK_SHIFT, g[1] >> BRICK_SHIFT],
        [(g[0] & m) as u32, (g[1] & m) as u32],
    )
}

/// Where level `j` of a brick's levels begins among them, in entries. Each entry of level
/// `j`, counting from 0, holds the lowest and highest of the 2^j × 2^j cells under it, the
/// ground's and the ways' apart (`ground::Extent`); level 0 is each cell's own column
/// (`fill::mip`).
pub const fn bound_mip_at(j: u32) -> u32 {
    (4 * BRICK_CELLS - ((4 * BRICK_CELLS) >> (2 * j))) / 3
}

/// Entry `[x, y]` of level `j` among a brick's levels, each level laid as
/// its cells are (`idx_of`).
pub(crate) const fn mip_idx(j: u32, at: [u32; 2]) -> u32 {
    bound_mip_at(j) + idx_of(at)
}

/// Cell `[x, y]`'s index among a brick's cells, or a level's entries; this is the one place
/// it is worked out. It is Morton order, a quadtree's nodes laid out as the Z curve visits
/// them, so an entry's four children a level under it are four in a row and the cells around
/// a cell lie near it in memory, as a texture's tiling lays texels out for the GPU's 2D cache.
/// A cell at an even x and the one after it in x are two in a row.
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
pub const BRICK_MIPS: u32 = BRICK_SHIFT;
/// A level entry's words: its bounds (`ground::Extent`), each as a bound word.
pub const BOUND_ENTRY_WORDS: u32 = 2;
pub const BOUND_WORDS: u32 = BOUND_ENTRY_WORDS * bound_mip_at(BRICK_MIPS + 1);

/// A tile's ways are cut into bins, `WAYS_ACROSS` a side, each `1 << WAYS_SHIFT` cells. A
/// brick lays its lines out by bin (`LINES_ENTRIES_AT`), and its fill keeps its columns'
/// spans from them.
pub const WAYS_ACROSS: u32 = 32;
pub const WAYS_SHIFT: u32 = (TILE_CELLS / WAYS_ACROSS).trailing_zeros();
pub const WAYS_BINS_ACROSS: u32 = BRICK >> WAYS_SHIFT;
pub const WAYS_BINS: u32 = WAYS_BINS_ACROSS * WAYS_BINS_ACROSS;

/// The most entries a brick holds of its tile's ways: one for each segment and each bin
/// (`WAYS_SHIFT`) its footprint reaches. It is the densest brick of the format-12 set; a tile
/// past it fails its bake (pxl-spec bake/tile/terrain.rs `ways_fit`).
pub const WAYS_BRICK_SEGMENTS: u32 = 1065;

/// A brick's lines, laid out by the CPU as the brick is filled (pxl-spec bake/tile/terrain.rs
/// `brick_lines`) so that its fill can keep its columns' spans from them (`fill::spans`).
/// First comes its tile's header (`way::header`); then one word a bin (`WAYS_BINS`), its first
/// entry with its count above it; then the entries, two to a word, each one of the brick's
/// segments, a bin's in order; then the segments the bins reach, in their lines' order,
/// `way::SEGMENT_WORDS` each. No brick holds more than `WAYS_BRICK_SEGMENTS` of either.
pub const LINES_ENTRIES_AT: u32 = way::HEADER_WORDS + WAYS_BINS;
pub const LINES_SEGMENTS_AT: u32 = LINES_ENTRIES_AT + WAYS_BRICK_SEGMENTS.div_ceil(2);
pub const LINES_WORDS: u32 = LINES_SEGMENTS_AT + WAYS_BRICK_SEGMENTS * way::SEGMENT_WORDS as u32;

/// A brick's scratch while it is filled: the spans of its columns and of the ring of columns
/// around it, which a column's cells read beside its own (pxl-engine fill.rs `spans`). The
/// ways' bound in its levels and its ways' tree are made from them (fill.rs `mip`,
/// `way::tree::leaf`), and the scratch is dropped with the fill.
pub const SPANS_AT: u32 = 0;
/// The columns whose spans a brick's cells read: the brick's and a ring of one around it,
/// `SPANS_ACROSS` a side, row by row from the ring's first.
pub const SPANS_ACROSS: u32 = BRICK + 2;
pub const SPANS_COLUMNS: u32 = SPANS_ACROSS * SPANS_ACROSS;
/// A column's spans in words (`way::SpanList`). The head word holds the count, with the bit
/// above it set where the spans did not fit `way::SPANS`, and above that the face cell as an
/// `i16`, `i16::MIN` for none (fill.rs `header_word`). Then each span holds its lowest and
/// past its highest, an `i16` each as the world's z is and held to that range
/// (`answer::bound_word`), and its value.
pub const SPANS_COLUMN_WORDS: u32 = 1 + 2 * way::SPANS as u32;
pub const SPANS_WORDS: u32 = SPANS_AT + SPANS_COLUMNS * SPANS_COLUMN_WORDS;

/// A brick's columns, one word a cell, row by row: a cell's ground top in leaf cells, an
/// `i16` in the low half (`column_top`).
pub(crate) const COLUMN_WORDS: u32 = BRICK_CELLS;

/// The ground top held in a column word: a signed 16-bit value in the word's low half.
pub const fn column_top(w: u32) -> i32 {
    (w << 16) as i32 >> 16
}

/// A brick's cells as its fill lays them, four to a word, cell `i` in the `i % 4`-th byte
/// (`own_at`): each cell's own palette index (`ground::own` of its four at the brick's
/// zoom).
pub const OWN_WORDS: u32 = BRICK_CELLS / OWN_CELLS;
/// The cells a laid word holds, a byte each: in the cells' order
/// (`idx_of`), a 2×2 square.
pub const OWN_CELLS: u32 = 4;
const _: () = assert!(BRICK_CELLS.is_multiple_of(OWN_CELLS));

/// Cell `i`'s palette index in a laid word `w`.
pub(crate) const fn own_at(w: u32, i: u32) -> u32 {
    (w >> (8 * (i % OWN_CELLS))) & 0xff
}

/// A brick's words, the part that outlives its fill, the same on the CPU and on a GPU, in
/// order: its columns, its levels, its cells as laid and its ways' tree's root. The clip
/// levels' bricks are one storage buffer bound whole on a GPU, under WebGPU's floor
/// (pxl-render gpu/ray/ground.rs `GROUND_BYTES`).
pub const COLUMNS_AT: u32 = 0;
pub const BOUNDS_AT: u32 = COLUMNS_AT + COLUMN_WORDS;
pub const OWN_AT: u32 = BOUNDS_AT + BOUND_WORDS;
/// Its ways' tree's root word: the child group of its own nodes along z in the pool and the
/// first one's Z (`way::tree::root_word`); 0, child group 0, which is never handed out, where
/// it has none.
pub const ROOT_AT: u32 = OWN_AT + OWN_WORDS;
pub const BRICK_WORDS: u32 = ROOT_AT + 1;
/// Which bricks the clip levels hold present, which is all `step` reads: the clip levels'
/// table alone.
pub trait Presence {
    /// The slot brick `q` at `cell_zoom` is held present in, which the reads
    /// of `BrickSource` take (`Entry::slot`); false where no clip level holds it.
    fn present(&self, cell_zoom: Zoom, q: [i32; 2]) -> (u32, bool);
}

/// The clip levels as one side, the CPU or the GPU, holds them: for each clip level, a
/// zoom's cells around its center, which brick each entry holds once it is present
/// (`Presence`), and the slots their cells are held in.
pub trait BrickSource: Presence {
    /// The brick the clip level read at `cell_zoom` wants in `entry`, and whether it is present;
    /// the inverse of `present`.
    fn brick(&self, cell_zoom: Zoom, entry: u32) -> ([i32; 2], bool);
    /// The four palette indices of cell `i` of the brick in `slot` (`ground::palette_idxs`).
    fn palette_word(&self, cell_zoom: Zoom, slot: u32, i: u32) -> u32;
    /// Column word `i` of the brick in `slot` (`COLUMN_WORDS`), `i` to
    /// `BRICK_CELLS`.
    fn column(&self, cell_zoom: Zoom, slot: u32, i: u32) -> u32;
    /// Entry `at` of level `j` of the brick in `slot`: its bounds (`ground::Extent`) as bound
    /// words (`answer::bound_word`), its columns' bounds joined (`join`), level 0 being one
    /// column's.
    fn bound_mip(&self, cell_zoom: Zoom, slot: u32, j: u32, at: [u32; 2]) -> [i32; 2];
    /// Cell `i`'s own palette index as its fill laid it (`OWN_WORDS`,
    /// `own_at`).
    fn own(&self, cell_zoom: Zoom, slot: u32, i: u32) -> u32;
    /// Entry `e` of the levels over the bricks of the clip level read at `cell_zoom`
    /// (`JOINED_WORDS`, `fold`), as this side folded them over the bricks it holds; the zero
    /// entry, meaning none, where it holds no fold.
    fn joined(&self, cell_zoom: Zoom, e: u32) -> [u32; JOINED_WORDS as usize];
    /// The first brick of the clip level read at `cell_zoom`, from which its blocks of bricks
    /// are counted (`fold`), and whether the level is laid out (`ORIGIN_AT`).
    fn origin(&self, cell_zoom: Zoom) -> ([i32; 2], bool);
    /// The root word of the ways' tree of the brick in `slot` (`ROOT_AT`), 0 for none.
    fn root(&self, cell_zoom: Zoom, slot: u32) -> u32;
}

/// One brick as its fill reads it, for the ways' one rule (`way::ColumnGrid`): its cells' zoom;
/// where it lies in its tile, in bricks; its first cell as the world names it, with X wrapped
/// there; what is known of the area at it (`area::Shard`), by which its columns are asked
/// whether they are the wall (`area::is_wall`); word `k` of its lines (`LINES_ENTRIES_AT`);
/// cell `i`'s own palette index as `columns` laid it (`OWN_WORDS`); and column `c`'s height
/// from its tile's samples, with `c` counted from its first cell and the ring of columns
/// around it included. Its columns are its cells and that ring. A ring column is read from
/// the edge bin it borders, whose lines hold every line that reaches it (`way::footprint`); a
/// column past the ring has no segment.
pub trait Brick {
    fn cell_zoom(&self) -> Zoom;
    fn place(&self) -> [u32; 2];
    fn first(&self) -> [i32; 2];
    fn shard(&self) -> Shard;
    fn line(&self, k: u32) -> u32;
    /// Cell `i`'s own palette index as its fill laid it (`OWN_WORDS`).
    fn own(&self, i: u32) -> u32;
    fn height(&self, c: [i32; 2]) -> i32;
}

/// Whether column `c` is the brick's or its ring's.
fn in_brick(c: [i32; 2]) -> bool {
    c[0] >= -1 && c[1] >= -1 && c[0] <= BRICK as i32 && c[1] <= BRICK as i32
}

/// Column `c`'s cell index in the brick, none outside it.
fn cell_idx(c: [i32; 2]) -> u32 {
    if c[0] < 0 || c[1] < 0 || c[0] >= BRICK as i32 || c[1] >= BRICK as i32 {
        return BRICK_CELLS;
    }
    idx_of([c[0] as u32, c[1] as u32])
}

/// Column `c`'s bin word, which for a ring column is the edge bin it borders: its first entry
/// and its count; none past the ring.
fn bin_of(sq: &impl Brick, c: [i32; 2]) -> (u32, u32) {
    if !in_brick(c) {
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

/// Entry `e` of the brick's lines: the place of a segment among the brick's segments.
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

fn header_of(sq: &impl Brick) -> (f32, StationGrid) {
    let mut w = [0; way::HEADER_WORDS as usize];
    let mut k = 0;
    while k < w.len() {
        w[k] = sq.line(k as u32);
        k += 1;
    }
    way::of_header(w)
}

/// A brick's columns as its lines and its cells give them.
impl<Q: Brick> ColumnGrid for Q {
    fn cell_zoom(&self) -> Zoom {
        Brick::cell_zoom(self)
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
    /// In eighths of its tile's cells: the brick's first cell, offset to the cell's middle.
    fn center(&self, c: [i32; 2]) -> [i32; 2] {
        let p = self.place();
        let eighths = way::POINT_SCALE as i32;
        let center = (lattice::CELL_CENTER * way::POINT_SCALE) as i32;
        [
            ((p[0] * BRICK) as i32 + c[0]) * eighths + center,
            ((p[1] * BRICK) as i32 + c[1]) * eighths + center,
        ]
    }
    /// The wall's one rule (`area::is_wall`), asked of the ring's columns too.
    fn is_walled(&self, c: [i32; 2]) -> bool {
        let (cell_zoom, first) = (Brick::cell_zoom(self), self.first());
        let b = [wrap_idx(cell_zoom, first[0] + c[0]), first[1] + c[1]];
        area::is_wall(self.shard(), cell_zoom, b)
    }
    fn stations(&self) -> StationGrid {
        header_of(self).1
    }
    /// Its own palette index as its fill laid it (`OWN_WORDS`), the wall
    /// apart (`is_walled`).
    fn palette_idx(&self, c: [i32; 2]) -> u32 {
        let i = cell_idx(c);
        if i == BRICK_CELLS {
            VOID
        } else {
            self.own(i)
        }
    }
    fn face(&self, c: [i32; 2]) -> i32 {
        if !in_brick(c) {
            return 0;
        }
        let cell_zoom = Brick::cell_zoom(self);
        ground::face_height(cell_zoom, ground::cells(cell_zoom, self.height(c) as f32))
    }
    /// Whether the entry is the one after the one before it, and its segment continues the one
    /// stored before it.
    fn joins(&self, c: [i32; 2], k: u32) -> bool {
        if k == 0 {
            return false;
        }
        let first = bin_of(self, c).0;
        let e = entry_of(self, first + k);
        e == entry_of(self, first + k - 1) + 1 && segment_at(self, e).continues == 1
    }
}

/// Where a node's cells at `cell_zoom`, a brick's worth or fewer, are held, as `step` finds
/// them: the zoom they were asked at, and the clip level holding them present (its zoom
/// `from`, at `cell_zoom` or coarser, and the slot), or none. It is one word, so that a
/// descent can carry it down beside what is placed (`object::NodeSlot`). `step` reads only
/// the brick at `cell_zoom` holding the cells and the bricks above it, so every node whose
/// cells lie in that brick has this answer, and a descent hands it from a node to the nodes
/// under it (`step`). `NONE` was asked at no zoom.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Lookup(u32);

/// The bits of a slot (`SLOTS`), which a GPU and the CPU name alike.
pub const LOOKUP_SLOT_BITS: u32 = SLOTS.next_power_of_two().trailing_zeros();
const LOOKUP_HERE: u32 = 1 << LOOKUP_SLOT_BITS;
pub const LOOKUP_FROM_SHIFT: u32 = LOOKUP_SLOT_BITS + 1;
pub const LOOKUP_CELL_ZOOM_SHIFT: u32 = LOOKUP_FROM_SHIFT + crate::ZOOM_BITS;
const LOOKUP_ZOOM_MASK: u32 = (1 << crate::ZOOM_BITS) - 1;
/// The bits of a `Lookup`'s word: its slot, whether it is held here, and its two zooms.
const LOOKUP_BITS: u32 = LOOKUP_CELL_ZOOM_SHIFT + crate::ZOOM_BITS;
const _: () = assert!(LOOKUP_BITS <= 32);
const _: () = assert!(
    FIRST_CELL_ZOOM >= 2,
    "a zoom asked is never NONE's, nor one finer"
);

impl Lookup {
    pub const NONE: Lookup = Lookup(0);

    const fn of(cell_zoom: Zoom, from: Zoom, slot: u32, here: bool) -> Lookup {
        Lookup(
            cell_zoom << LOOKUP_CELL_ZOOM_SHIFT
                | from << LOOKUP_FROM_SHIFT
                | (here as u32) << LOOKUP_SLOT_BITS
                | slot,
        )
    }
    /// The zoom its cells were asked at.
    pub const fn cell_zoom(self) -> Zoom {
        self.0 >> LOOKUP_CELL_ZOOM_SHIFT
    }
    /// The clip level's zoom.
    pub const fn from(self) -> Zoom {
        (self.0 >> LOOKUP_FROM_SHIFT) & LOOKUP_ZOOM_MASK
    }
    pub const fn slot(self) -> u32 {
        self.0 & (LOOKUP_HERE - 1)
    }
    pub const fn here(self) -> bool {
        self.0 & LOOKUP_HERE != 0
    }
    pub const fn word(self) -> u32 {
        self.0
    }
    pub(crate) const fn of_word(w: u32) -> Lookup {
        Lookup(w)
    }
}

/// The cell at `from` that column `c` of the cells at `zoom` lies in.
fn cell_at(zoom: Zoom, from: Zoom, c: [i32; 2]) -> [i32; 2] {
    let s = zoom - from;
    [wrap_idx(from, c[0] >> s), c[1] >> s]
}

/// The clip level holding a node's cells at `cell_zoom`, from its first cell `g`, stepped
/// from `above`, what a node over it found, or `NONE`; it reads only what `above` leaves
/// unknown. Only a node a brick wide or less sets a place (`bounded`), so every node a
/// descent asks under it, a sibling read from its parent's stack entry included, lies in that
/// node, and at each zoom in `above`'s cell, in the brick that held `above`'s cells there. The
/// clip levels are asked finest first, one a zoom, and the first holding the cell present
/// answers, so `above` says what each of them answers: none present above `above`'s clip
/// level, and that one present. Asked at `above`'s cell zoom or coarser, down to its clip
/// level, the answer is that clip level, and nothing is read. Asked finer, only the zooms
/// finer than `above`'s are unknown: they are looked at finest first, and then `above`'s
/// answer stands, one look a zoom of the step. With nothing above, or coarser than `above`'s
/// clip level, every clip level from `cell_zoom`'s to the first (`FIRST_CELL_ZOOM`) is looked
/// at, and none holding the cell is a lack.
pub fn step(w: &impl Presence, above: Lookup, cell_zoom: Zoom, g: [i32; 2]) -> (Lookup, [u32; 2]) {
    // The zooms `above` leaves unknown, finest first, down to the one it answers, or down to the
    // first clip level where it answers none.
    let known = above != Lookup::NONE && cell_zoom >= above.from();
    let (floor, mut f) = if known {
        (
            above.cell_zoom(),
            Lookup::of(cell_zoom, above.from(), above.slot(), above.here()),
        )
    } else {
        (
            FIRST_CELL_ZOOM - 1,
            Lookup::of(cell_zoom, FIRST_CELL_ZOOM, 0, false),
        )
    };
    let mut c = cell_zoom;
    while c > floor {
        let (slot, here) = w.present(c, brick_of(cell_at(cell_zoom, c, g)).0);
        if here {
            f = Lookup::of(cell_zoom, c, slot, true);
            break;
        }
        c -= 1;
    }
    (f, brick_of(cell_at(cell_zoom, f.from(), g)).1)
}

/// The zoom a cell at `cell_zoom` is read at, among the eye's clip levels' (`FIRST_CELL_ZOOM`
/// to `LEAF_CELL_ZOOM`): a leaf's zoom finer than the leaves, and the coarsest clip level's
/// coarser than that level.
pub const fn clip_zoom(cell_zoom: Zoom) -> Zoom {
    if cell_zoom > LEAF_CELL_ZOOM {
        LEAF_CELL_ZOOM
    } else if cell_zoom < FIRST_CELL_ZOOM {
        FIRST_CELL_ZOOM
    } else {
        cell_zoom
    }
}

/// The ground's cell `g` at `cell_zoom`, read from the finest clip level holding it present,
/// stepped from `place` (`step`): its four palette indices, its column's word (its height),
/// its own palette index as laid, and where it is held; void at sea level where none holds
/// it.
pub fn cell(
    w: &impl BrickSource,
    place: Lookup,
    cell_zoom: Zoom,
    g: [i32; 2],
) -> (u32, u32, u32, Lookup) {
    let (f, at) = step(w, place, cell_zoom, g);
    if !f.here() {
        return (VOID, 0, VOID, f);
    }
    let (from, slot, i) = (f.from(), f.slot(), idx_of(at));
    (
        w.palette_word(from, slot, i),
        w.column(from, slot, i),
        w.own(from, slot, i),
        f,
    )
}

/// Column `b` at `zoom` as a reader reads it: the cell drawing it (a leaf's, finer than the
/// leaves), stepped from `place`, where the column's own cells were found (`step`), with its
/// own palette index as laid, or the wall's (`UNKNOWN`) where it is the wall
/// (`area::is_wall`); and where it is held. The wall is read from `shard`, what is known of
/// the area there, or else asked of `w` (`Shard::at`).
pub fn ground(
    w: &(impl BrickSource + Area),
    shard: Shard,
    place: Lookup,
    zoom: Zoom,
    b: [i32; 2],
) -> (Cell, Lookup) {
    let read = clip_zoom(zoom);
    let (word, column, own, f) = cell(w, place, read, cell_at(zoom, read, b));
    let palette_idx = if area::is_wall(shard.at(w, (zoom, b)), zoom, b) {
        UNKNOWN
    } else {
        own
    };
    let (top, part) = ground::top(zoom, column_top(column) as f32);
    let cell = Cell {
        palette_idx,
        top,
        part,
        fold: ground::palette_idxs(word),
    };
    (cell, f)
}

/// Entry `at` of level `j` of a brick, counting from 1, over the level under it, which is
/// written first: the join of the four entries under it, each source's on its own
/// (`ground::joined`), as bound words.
pub fn join(
    w: &impl BrickSource,
    cell_zoom: Zoom,
    slot: u32,
    j: u32,
    [x, y]: [u32; 2],
) -> [i32; 2] {
    let under = |q: u32| {
        ground::extent_of(w.bound_mip(cell_zoom, slot, j - 1, [2 * x + q % 2, 2 * y + q / 2]))
    };
    let mut b = under(0);
    let mut q = 1;
    while q < 4 {
        b = ground::joined(b, under(q));
        q += 1;
    }
    ground::extent_word(b)
}

/// The levels over which a clip level bounds a node: its bricks' own levels, then the levels
/// joining bricks, up to the whole clip level.
pub(crate) const BOUND_MIPS: u32 = BRICK_MIPS + ACROSS.trailing_zeros();

/// The levels over a clip level's bricks, from one level over a brick (`BRICK_MIPS + 1`) to
/// the whole clip level. A node wider than a brick is bounded by the join of the bricks it
/// covers, folded where the clip levels' entries are written (`fold`) and read as one entry
/// (`bounded`).
pub const JOINED_MIPS: u32 = BOUND_MIPS - BRICK_MIPS;
/// A clip level's entries: level `l` over a brick has `(ACROSS >> l)²`, an
/// aligned block of `2^l` bricks a side each.
pub const JOINED_ENTRIES: u32 = joined_idx(JOINED_MIPS + 1, [0, 0]);
/// An entry's words: its key, the block's first brick at its cells' zoom, X then Y, each with
/// its sign flipped (`key_word`) so that the zero entry, as fresh memory holds it, names no
/// brick; then the join's bounds, the ground's and the ways' (`ground::extent_word`). Each
/// entry is a row of the clip levels' buffer, an `IVec4` as the rows of their entry table
/// are, placed after that table (`JOINED_AT`).
pub const JOINED_WORDS: u32 = 4;
/// A row of the clip levels' table, as the core writes it (pxl-core
/// domain/layer/terrain/clipmap.rs `table`) and the GPU reads it (pxl-render clip.wgsl), an
/// `IVec4` of its buffer: the brick an entry's clip level wants there, and, where its cells
/// are present, the slot holding them plus one (`slot`), or 0 while they are not. Another
/// clip level at the same zoom may have filled that slot. The fourth word holds nothing.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Entry {
    pub brick: [i32; 2],
    pub present: i32,
    row: i32,
}

const _: () = assert!(size_of::<Entry>() == 16, "an entry is a row of four words");

impl Entry {
    pub const fn absent(brick: [i32; 2]) -> Entry {
        Entry::at(brick, 0)
    }

    /// Its brick, and its slot plus one where it is present (`slot`).
    pub const fn at(brick: [i32; 2], present: i32) -> Entry {
        Entry {
            brick,
            present,
            row: 0,
        }
    }

    /// The row the buffer's four words hold.
    pub fn of([x, y, z, _]: [i32; 4]) -> Entry {
        Entry::at([x, y], z)
    }

    /// Its brick, and whether present (`BrickSource::brick`).
    pub fn brick(&self) -> ([i32; 2], bool) {
        (self.brick, self.present != 0)
    }

    /// Whether it holds `q` present (`BrickSource::present`).
    pub fn holds(&self, q: [i32; 2]) -> bool {
        self.present != 0 && self.brick[0] == q[0] && self.brick[1] == q[1]
    }

    /// The slot its brick's cells are held in, while present.
    pub fn slot(&self) -> u32 {
        (self.present - 1) as u32
    }

    /// Whether a query, or what one handed on, reads this entry differently from `was`: another
    /// brick or another slot. A brick that is wanted but absent is read too: its cells come from
    /// a coarser clip level, where its block's bound is folded (`fold`).
    pub fn changed_from(&self, was: Entry) -> bool {
        self.brick[0] != was.brick[0]
            || self.brick[1] != was.brick[1]
            || self.present != was.present
    }

    /// Whether a query that reads only bricks held present (`Presence`) reads this entry
    /// differently from `was`: it moved (`changed_from`) and is held present on either side. A
    /// brick wanted absent in place of another moves only what reads the fold.
    pub fn present_changed_from(&self, was: Entry) -> bool {
        self.changed_from(was) && (self.present != 0 || was.present != 0)
    }
}

/// The row of the clip levels' buffer at which each clip level's first brick lies: past
/// every clip level's entries, one row a clip level, shaped as an entry (`Entry`), with the
/// clip level's first brick as its brick and `present` 1 while the level is laid out
/// (`BrickSource::origin`).
pub const ORIGIN_AT: u32 = SLOTS;
/// The row of the clip levels' buffer the eye's clip levels' entries begin at:
/// past their entries and their first bricks.
pub const JOINED_AT: u32 = ORIGIN_AT + CLIP_LEVELS;
/// The eye's clip levels' entries, as the GPU lays them: clip level `cell_zoom −
/// FIRST_CELL_ZOOM` a run of `JOINED_ENTRIES`.
pub const EYE_JOINED_WORDS: u32 = JOINED_ENTRIES * JOINED_WORDS * EYE_CLIP_LEVELS;
/// The clip levels' buffer in words: its entries, its first bricks and the eye's clip levels'
/// joined levels, all of it written by the core (pxl-core domain/layer/terrain/clipmap.rs
/// `table`).
pub const TABLE_WORDS: u32 = 4 * JOINED_AT + EYE_JOINED_WORDS;

/// The index, among a clip level's entries, of the entry of level `l` (from 1) over a brick
/// whose block is `at`, counted in blocks of that level. The levels' entries come in order,
/// each level's in Morton order (`idx_of`), so an entry's four children are the entries
/// `4b..4b + 4` of the level under it. The levels under `l` hold `(ACROSS >> m)²` each, `m`
/// from 1, a geometric sum of `(ACROSS² − (ACROSS >> (l − 1))²) / 3`, which is whole as a
/// difference of two powers of four.
pub const fn joined_idx(l: u32, at: [u32; 2]) -> u32 {
    let under = ACROSS >> (l - 1);
    (ACROSS * ACROSS - under * under) / 3 + idx_of(at)
}
const _: () = assert!(ACROSS.is_power_of_two() && JOINED_MIPS < ACROSS.trailing_zeros() + 1);

/// A key's word: `v` with its sign flipped, so no brick's is 0.
const fn key_word(v: i32) -> u32 {
    v as u32 ^ 1 << 31
}

/// Entry `b` of level `l` over a brick (its Morton index there, `joined_idx`) of the clip
/// level read at `cell_zoom`, folded from the level under it. It is what a ray reads at
/// `cell_zoom` in the block of `2^l` entries a side at `b`, aligned as the entries are
/// (`ACROSS` is a multiple of the block's side): the join of its four children's joins, each
/// read once where the level under it laid it (`BrickSource::joined`; at level 1, a brick's own,
/// `brick_bounds`). It is keyed by the clip level's first brick (`BrickSource::origin`), so an entry
/// of a clip level that has since moved names another brick. A block within the clip level's
/// range is a node's bricks; one across its seam, where the entries wrap, holds what the clip
/// level holds of the two nodes on either side, which bounds either from above. It is the
/// zero entry where the clip level is not laid out or no clip level holds a brick of the
/// block, a lack. A level is folded once the one under it is.
pub fn fold(w: &impl BrickSource, cell_zoom: Zoom, l: u32, b: u32) -> [u32; JOINED_WORDS as usize] {
    let [x, y] = of_idx(b);
    let (first, laid) = w.origin(cell_zoom);
    let key = [key_word(first[0]), key_word(first[1])];
    let half = 1 << (l - 1);
    let mut joined = [ground::EMPTY; 2];
    let mut k = 0;
    while laid && k < 4 {
        let (dx, dy) = ((k & 1) * half, (k >> 1) * half);
        let got = if l == 1 {
            let (q, _) = w.brick(
                cell_zoom,
                entry([((x << l) + dx) as i32, ((y << l) + dy) as i32]),
            );
            brick_bounds(w, cell_zoom, q)
        } else {
            unfold(
                w.joined(cell_zoom, joined_idx(l - 1, [0, 0]) + 4 * b + k),
                first,
            )
        };
        // Compared component by component, because a GPU has no array equality. A child that no
        // clip level holds makes the entry none for both sources.
        if got[0][0] == ground::UNBOUNDED[0] && got[0][1] == ground::UNBOUNDED[1] {
            return [0; JOINED_WORDS as usize];
        }
        joined = ground::joined(joined, got);
        k += 1;
    }
    if !laid {
        return [0; JOINED_WORDS as usize];
    }
    let [g, ways] = ground::extent_word(joined);
    [key[0], key[1], g as u32, ways as u32]
}

/// What a ray reads of brick `q` at `cell_zoom`: its cells from the finest clip level holding
/// them present, this one or a coarser one, as a node a brick wide reads them (`bounded`), so
/// a brick this clip level has yet to hold is bounded by the cells read in its place; none
/// (`ground::UNBOUNDED`) where no clip level holds it, a lack. Only the bricks' own levels are
/// read, never another clip level's entries, so the clip levels may be folded in any order.
fn brick_bounds(w: &impl BrickSource, cell_zoom: Zoom, q: [i32; 2]) -> Extent {
    let mut place = Lookup::NONE;
    let (b, _, here, _) = bounded(w, &mut place, cell_zoom, (cell_zoom - BRICK_MIPS, q));
    if here {
        b
    } else {
        [ground::UNBOUNDED; 2]
    }
}
const _: () = assert!(
    FIRST_CELL_ZOOM >= BRICK_MIPS,
    "a brick is a node at its cells' zoom less `BRICK_MIPS`"
);

/// The joins that entry `e` holds for a clip level whose first brick is `q` (`fold`): its
/// bounds where its key is `q`, or else none (`ground::UNBOUNDED`), for the zero entry and a
/// moved clip level's entry alike.
fn unfold(e: [u32; JOINED_WORDS as usize], q: [i32; 2]) -> Extent {
    if e[0] == key_word(q[0]) && e[1] == key_word(q[1]) {
        ground::extent_of([e[2] as i32, e[3] as i32])
    } else {
        [ground::UNBOUNDED; 2]
    }
}

/// What the clip level read at `cell_zoom` holds of a node wider than a brick, whose first
/// cell there is `g` and which is `2^l` bricks a side, `l` from 1: one entry, its block's
/// (`fold`), which is the node's bricks exactly where the node lies in the clip level's
/// range, and a superset where it crosses the seam; for a node wider than the clip level, the
/// whole clip level, the top block. With it, whether no other clip level needs to be read: the
/// node lies in the clip level, or the entry is none (`ground::UNBOUNDED`) because a brick of
/// its block is held by no clip level, so nothing bounds the node, which only splits more.
/// What the clip level does not reach of the node, a coarser clip level holds, as for a cell
/// (`step`). This is the one reader of every entry.
fn level_bounds(w: &impl BrickSource, cell_zoom: Zoom, g: [i32; 2], l: u32) -> (Extent, bool) {
    let (first, laid) = w.origin(cell_zoom);
    if !laid {
        return ([ground::EMPTY; 2], false);
    }
    let (q, _) = brick_of([wrap_idx(cell_zoom, g[0]), g[1]]);
    // The node's first brick counted from the clip level's, with X the short way around.
    let d = [
        wrap_idx(bricks_zoom(cell_zoom), q[0] - first[0]),
        q[1] - first[1],
    ];
    let (side, across) = (1i32 << l, ACROSS as i32);
    if d[0] >= across || d[1] >= across || d[0] + side <= 0 || d[1] + side <= 0 {
        return ([ground::EMPTY; 2], false);
    }
    let all = l <= JOINED_MIPS
        && d[0] >= 0
        && d[1] >= 0
        && d[0] + side <= across
        && d[1] + side <= across;
    let (mip, m) = (
        if l > JOINED_MIPS { JOINED_MIPS } else { l },
        (ACROSS - 1) as i32,
    );
    let at = [((q[0] & m) as u32) >> mip, ((q[1] & m) as u32) >> mip];
    let b = unfold(w.joined(cell_zoom, joined_idx(mip, at)), first);
    // A brick no clip level holds is bounded by no coarser clip level either.
    if b[0][0] == ground::UNBOUNDED[0] && b[0][1] == ground::UNBOUNDED[1] {
        return (b, true);
    }
    (b, all)
}

/// What the clip levels hold of the node `(zoom, c)` at the cells from `coarsest` to
/// `finest`, each held to the clip levels' zooms (`clip_zoom`). The clip level at each zoom is
/// read once, finest first, `finest`'s stepped from `place`, which becomes where its cells
/// are held; a read from a coarser clip level stands for every zoom down to it. Under
/// `coarsest` the reads go on, while no clip level read there holds all of the node, in a
/// cell's own order (`step`), each zoom's cells being its own tile's, which no other zoom's
/// bound holds. It returns the lowest and highest joined, and the zoom of the finest's clip
/// level; none (`ground::UNBOUNDED`) where nothing bounds the node, which only splits more;
/// and false where a clip level it needs holds none of the node, a lack. This is the one way
/// a bound over a node is read.
pub fn bound_over(
    w: &impl BrickSource,
    place: &mut Lookup,
    [coarsest, finest]: [Zoom; 2],
    (zoom, c): (Zoom, [i32; 2]),
) -> (Extent, Zoom, bool) {
    let (top, bottom) = (clip_zoom(finest), clip_zoom(coarsest));
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
        // A read from a coarser clip level than asked stands for every zoom down to it: none
        // between holds the node's brick, so each would read it again.
        let open = b[0][0] <= ground::UNBOUNDED[0] && b[0][1] >= ground::UNBOUNDED[1];
        if open || from <= bottom && all || from == FIRST_CELL_ZOOM {
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

/// The lowest and highest that the cells at `cell_zoom` reach under the node `(zoom, c)`.
/// For a node no wider than a brick, it reads the finest clip level holding them present,
/// stepped from `place`, which becomes where they are held (`step`): one entry of its brick's
/// levels, level 0 being the column holding a node no wider than a cell, with that clip
/// level's zoom. For a node wider than a brick, it reads what the clip level at `cell_zoom`
/// holds of it (`level_bounds`), and whether it holds all of it (`bound_over` reads on where it
/// does not). It is false where no clip level holds the node, a lack.
fn bounded(
    w: &impl BrickSource,
    place: &mut Lookup,
    cell_zoom: Zoom,
    (zoom, c): (Zoom, [i32; 2]),
) -> (Extent, Zoom, bool, bool) {
    // The node's first cell at `cell_zoom`, or the cell holding it.
    let g = [
        crate::idx_at(zoom, c[0], cell_zoom),
        crate::idx_at(zoom, c[1], cell_zoom),
    ];
    if cell_zoom > zoom + BRICK_MIPS {
        let (b, all) = level_bounds(w, cell_zoom, g, cell_zoom - zoom - BRICK_MIPS);
        return (b, cell_zoom, true, all);
    }
    let (f, p) = step(w, *place, cell_zoom, g);
    *place = f;
    let (from, slot) = (f.from(), f.slot());
    if !f.here() {
        return ([[0, 0]; 2], from, false, false);
    }
    let (j, at) = entry_at(zoom, from, p);
    (
        ground::extent_of(w.bound_mip(from, slot, j, at)),
        from,
        true,
        true,
    )
}

/// The level of a brick at `from` that bounds a node at `zoom`, level 0 being a column's own
/// for a node no wider than a cell, and the entry in it for the node's first cell, at
/// `[x, y]` in its brick.
fn entry_at(zoom: Zoom, from: Zoom, [x, y]: [u32; 2]) -> (u32, [u32; 2]) {
    let j = from.saturating_sub(zoom);
    (j, [x >> j, y >> j])
}

#[cfg(test)]
mod tests {
    use super::Entry;

    #[test]
    fn changed_from_reads_what_a_query_reads() {
        let at = Entry::at;
        // Present: another brick or another slot is a move.
        assert!(at([1, 2], 3).changed_from(at([1, 5], 3)));
        assert!(at([1, 2], 3).changed_from(at([4, 2], 3)));
        assert!(at([1, 2], 3).changed_from(at([1, 2], 4)));
        assert!(!at([1, 2], 3).changed_from(at([1, 2], 3)));
        // Present on one side alone.
        assert!(at([1, 2], 3).changed_from(at([1, 2], 0)));
        assert!(at([1, 2], 0).changed_from(at([1, 2], 3)));
        // Neither present: another brick is a move.
        assert!(at([1, 2], 0).changed_from(at([8, 9], 0)));
        assert!(!at([1, 2], 0).changed_from(at([1, 2], 0)));
        // A move counts where the brick is held present on at least one side, and not where a brick
        // wanted absent replaces another.
        assert!(at([1, 2], 3).present_changed_from(at([8, 9], 0)));
        assert!(at([1, 2], 0).present_changed_from(at([8, 9], 3)));
        assert!(!at([1, 2], 0).present_changed_from(at([8, 9], 0)));
    }
}
