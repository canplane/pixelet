// pxl-engine/clipmap.rs

//! wgsl-port: the ground's cells as they are held, one clipmap level (Tanner 1998) a zoom,
//! around a center. A ray observes a zoom's cells only within `RADIUS` of those cells from
//! the eye (pxl-spec space/layer/demand.rs `band`; the top zoom's by its reach), so a clipmap
//! level `CLIP_SIZE` cells a side around the eye holds every cell it can observe at that
//! zoom. A clipmap level is `ACROSS` bricks a side, and each brick is held in the entry its
//! place names modulo `ACROSS`, so the level follows its center a brick at a time and no held
//! cell moves. Clipmap levels at one zoom hold a brick in the same entry, so a brick at a
//! zoom is held once, in a slot of that entry, however many clipmap levels want it (`Entry`).
//! A brick is present once its cells are drawn; a reader of an absent brick reads the
//! coarser clipmap level's (`step`).

use crate::answer;
use crate::ground;
use crate::ground::Cell;
use crate::ground::Extent;
use crate::way;
use pxl_spec::asset::cube::Sentinel;
use pxl_spec::brick::{BRICK, BRICK_CELL_CNT, BRICK_SHIFT, bricks_zoom, idx_of, of_idx};
use pxl_spec::ground::{GroundId, asset_ids, top};
use pxl_spec::space::camera::grid;
use pxl_spec::space::coord::world::wrap_idx;
use pxl_spec::space::coord::zoom::{LEAF_CELL_ZOOM, MIN_ZOOM, Zoom, idx_at};
use pxl_spec::space::hierarchy::tree::child_of;
use pxl_spec::space::layer::area;
use pxl_spec::space::layer::area::{Area, Shard};
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::way::{NOTHING, SPAN_CAP};

/// The bricks a clip level holds a side (`ACROSS`) and in all (`ENTRY_CNT`): enough to hold
/// `RADIUS` around a center anywhere in a brick, rounded up to a power of two, since `entry`
/// masks by it.
pub const ACROSS: u32 = ((2 * RADIUS + BRICK).div_ceil(BRICK)).next_power_of_two();
pub const ENTRY_CNT: u32 = ACROSS * ACROSS;
/// A clip level's side, counted in cells.
pub const CLIP_SIZE: u32 = ACROSS * BRICK;
/// The eye's coarsest clip level's cells' zoom: the top's.
pub const FIRST_CELL_ZOOM: Zoom = GROUND.cell_zoom(MIN_ZOOM);
/// The eye's clip levels, one for every zoom the pyramid draws, coarsest first, which the
/// GPU's cast reads; the one at `cell_zoom` is number `cell_zoom − FIRST_CELL_ZOOM`.
pub const EYE_CLIP_LEVEL_CNT: u32 = LEAF_CELL_ZOOM - FIRST_CELL_ZOOM + 1;

/// The cells' zoom of the eye's clip level `w`, counting coarsest first: the zoom the layout
/// lays the level at, and the zoom at which the GPU's decor reads a brick's clip level.
pub const fn eye_cell_zoom(w: u32) -> Zoom {
    FIRST_CELL_ZOOM + w
}
/// Every clip level: the eye's; the pivot's two, at the leaves' zoom and the one above, which
/// walking and editing read; and one for a destination, at the zoom its line reads the ground
/// at, found exactly (pxl-core domain/layer/terrain/clipmap.rs).
pub const CLIP_LEVEL_CNT: u32 = EYE_CLIP_LEVEL_CNT + 3;
/// The slots every clip level's bricks are held in, `CLIP_LEVEL_CNT` to an entry, slot
/// `k × ENTRY_CNT + s` being entry `s`'s `k`-th (pxl-core domain/layer/terrain/clipmap.rs
/// `Slot`). No more bricks want an entry than there are clip levels, so this is the most held
/// at once, reached where no two clip levels at a zoom overlap. A brick that two of them want
/// takes one slot.
pub const SLOT_CNT: u32 = CLIP_LEVEL_CNT * ENTRY_CNT;
/// The most bricks a frame hands the GPU once the core's fill has filled them (pxl-core
/// domain/layer/terrain/clipmap.rs `upload`). Each brick is a box of decor's generation
/// (pxl-content `generation::RECT_CAP`), and bricks past this many are handed over in later
/// frames.
pub const FRAME_BRICK_CAP: u32 = 9;

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
    (4 * BRICK_CELL_CNT - ((4 * BRICK_CELL_CNT) >> (2 * j))) / 3
}

/// Entry `[x, y]` of level `j` among a brick's levels, each level laid as
/// its cells are (`idx_of`).
pub(crate) const fn mip_idx(j: u32, at: [u32; 2]) -> u32 {
    bound_mip_at(j) + idx_of(at)
}

pub const BRICK_MIP_LAST: u32 = BRICK_SHIFT;
/// A level entry's words: its bounds (`ground::Extent`), each as a bound word.
pub const BOUND_ENTRY_WORDS: u32 = 2;
pub const BOUND_WORDS: u32 = BOUND_ENTRY_WORDS * bound_mip_at(BRICK_MIP_LAST + 1);

/// The words of the ways' pool, each brick's run of kept spans among them, as one side holds
/// them: the core's (pxl-core domain/layer/terrain/pool.rs) or a test's. A word past the store
/// reads as 0.
pub trait Pool {
    fn word(&self, k: u32) -> u32;
}

impl Pool for [u32] {
    fn word(&self, k: u32) -> u32 {
        crate::at(self, k)
    }
}

/// The most words the ways' pool holds, its budget on the heap (64 MiB) and its GPU copy's.
/// Its first word is never handed out, so a brick's `ROOT_AT` of 0 names no run.
pub const WAYS_POOL_WORDS: u32 = 1 << 24;

/// A brick's kept spans, a run of the ways' pool its fill writes (fill.rs `lay_spans`), read in
/// constant time with plain loads (`KeptSpans`): a bit per column of the brick and its ring
/// (`SPANS_COLUMN_CNT`, row by row from the ring's first, `SPANS_ACROSS` a row), set where the
/// column is kept; the columns set before each pair of bitmap words, an `i16` each, two to a
/// word, so a column's rank is that count and at most two words' bits; then, by rank, the word
/// its column begins at in the run. A column there is `SPANS_COLUMN_WORDS`' words for its
/// spans alone: the head word and, for each span, its lowest and past its highest and its
/// value. A column with no span is kept only where a bore's air beside it may line it (it
/// needs its face). A brick with no span takes no run.
pub const RUN_BITS_AT: u32 = 0;
pub const RUN_BITS_WORDS: u32 = SPANS_COLUMN_CNT.div_ceil(32);
pub const RUN_RANK_AT: u32 = RUN_BITS_AT + RUN_BITS_WORDS;
pub const RUN_RANK_WORDS: u32 = RUN_BITS_WORDS.div_ceil(2).div_ceil(2);
pub const RUN_OFFS_AT: u32 = RUN_RANK_AT + RUN_RANK_WORDS;
/// The most words a run takes: its index, and a word and a full column for each column.
pub const RUN_WORDS_MOST: u32 = RUN_OFFS_AT + SPANS_COLUMN_CNT * (1 + SPANS_COLUMN_WORDS);

/// The bits of a head word that hold a column's spans' count (`SPANS_COLUMN_WORDS`).
pub const SPANS_CNT: u32 = 0xffff;
/// A head word's face cell where the column has none (`SPANS_COLUMN_WORDS`).
pub const FACE_NONE: i32 = i16::MIN as i32;

/// A head word's face cell (`i32::MIN` for none) and spans' count, held to pxl-spec `way::SPAN_CAP` so
/// that a loop over them stays bounded whatever the word.
pub const fn head_of(w: u32) -> (i32, u32) {
    let face = w as i32 >> 16;
    (
        if face == FACE_NONE { i32::MIN } else { face },
        if w & SPANS_CNT < SPAN_CAP as u32 {
            w & SPANS_CNT
        } else {
            SPAN_CAP as u32
        },
    )
}

/// The spans a brick's run keeps, read as its fill kept them, for the one rule
/// (`way::way_cell`): a column the run does not keep has no spans and no face.
struct KeptSpans<'a, P: ?Sized> {
    pool: &'a P,
    run: u32,
}

impl<P: Pool + ?Sized> KeptSpans<'_, P> {
    /// Where column `c`'s spans begin in the run, or 0 where it is not kept.
    fn column(&self, c: [i32; 2]) -> u32 {
        if c[0] < -1 || c[1] < -1 || c[0] > BRICK as i32 || c[1] > BRICK as i32 {
            return 0;
        }
        let i = (c[1] + 1) as u32 * SPANS_ACROSS + (c[0] + 1) as u32;
        let (w, bit) = (i / 32, i % 32);
        let word = self.pool.word(self.run + RUN_BITS_AT + w);
        if word >> bit & 1 == 0 {
            return 0;
        }
        let rank = self.pool.word(self.run + RUN_RANK_AT + w / 4) >> (16 * ((w / 2) & 1));
        let before = if w & 1 == 1 {
            self.pool.word(self.run + RUN_BITS_AT + w - 1).count_ones()
        } else {
            0
        };
        let r = (rank & 0xffff) + before + (word & ((1u32 << bit) - 1)).count_ones();
        self.pool.word(self.run + RUN_OFFS_AT + r)
    }
}

impl<P: Pool + ?Sized> way::SpanTable for KeptSpans<'_, P> {
    fn hdr(&self, c: [i32; 2]) -> (i32, u32) {
        match self.column(c) {
            0 => (i32::MIN, 0),
            at => head_of(self.pool.word(self.run + at)),
        }
    }
    fn span(&self, c: [i32; 2], j: u32) -> ([i32; 2], u32) {
        let w = self.run + self.column(c) + 1 + 2 * j;
        (
            answer::bound_of(self.pool.word(w) as i32),
            self.pool.word(w + 1),
        )
    }
}

/// What the ways write in the node `(zoom, c)`, a cell at or finer than the brick's cells it was
/// read from, `place` naming that brick's slot (`clipmap::step`); pxl-spec `way::NOTHING` where
/// nothing writes it. It is the rule over the spans the brick's fill kept (`way::way_cell`), read
/// at the brick's cell holding the node, what a tree of its cells would have held at its leaf;
/// `ways`, the column's bound at level 0 (`ground::Extent`, `way::lined_hull`), turns down the
/// cells no span of it or beside it can write. A node coarser than the cells of the brick it was
/// read from writes none: a cell is asked no coarser than the coarsest clip level's (pxl-core
/// `domain/query/node.rs` `coarsest`, pxl-render's `eye::RANGE`, and a descent's `near`, held to
/// it), so `zoom` is never under the brick's cells it was read from, and the fold over the cells of
/// a coarser node is asked of nothing.
pub fn way_at<W: BrickSource, P: Pool + ?Sized>(
    bricks: &W,
    pool: &P,
    place: Lookup,
    ways: [i32; 2],
    (zoom, c): (Zoom, [i32; 3]),
) -> u32 {
    let from = place.from();
    if !place.here() || zoom < from {
        return NOTHING;
    }
    let shift = zoom - from;
    let z = c[2] >> shift;
    let s = 1i32 << (LEAF_CELL_ZOOM - from);
    if z * s >= ways[1] || (z + 1) * s <= ways[0] {
        return NOTHING;
    }
    let run = bricks.root(from, place.slot());
    if run == 0 {
        return NOTHING;
    }
    let (_, at) = brick_of([wrap_idx(from, c[0] >> shift), c[1] >> shift]);
    way::way_cell(
        &KeptSpans { pool, run },
        [at[0] as i32, at[1] as i32],
        from,
        z,
    )
}

/// A brick's scratch while it is filled: the spans of its columns and of the ring of columns
/// around it, which a column's cells read beside its own (pxl-engine fill.rs `spans`). The
/// ways' bound in its levels (fill.rs `mip`) and its run of kept spans (fill.rs `lay_spans`)
/// are made from them, and the scratch is dropped with the fill.
pub const SPANS_AT: u32 = 0;
/// The columns whose spans a brick's cells read: the brick's and a ring of one around it,
/// `SPANS_ACROSS` a side, row by row from the ring's first.
pub const SPANS_ACROSS: u32 = BRICK + 2;
pub const SPANS_COLUMN_CNT: u32 = SPANS_ACROSS * SPANS_ACROSS;
/// A column's spans in words (pxl-spec `way::SpanList`). The head word holds the count
/// (`SPANS_CNT`) and, above it, the face cell as an `i16`, `FACE_NONE` for none (fill.rs
/// `hdr_word`). Then each span holds its lowest and past its highest, an `i16` each as the
/// world's z is and held to that range (`answer::bound_word`), and its value.
pub const SPANS_COLUMN_WORDS: u32 = 1 + 2 * SPAN_CAP as u32;
pub const SPANS_WORDS: u32 = SPANS_AT + SPANS_COLUMN_CNT * SPANS_COLUMN_WORDS;

/// A brick's columns, one word a cell, row by row: a cell's ground top in leaf cells, an
/// `i16` in the low half (`column_top`).
pub(crate) const COLUMN_WORDS: u32 = BRICK_CELL_CNT;

/// The ground top held in a column word: a signed 16-bit value in the word's low half.
pub const fn column_top(w: u32) -> i32 {
    (w << 16) as i32 >> 16
}

/// A brick's cells as its fill lays them, `OWN_CELL_CNT` to a word, cell `i` in the word's
/// `i % OWN_CELL_CNT`-th `GroundId` (`own_at`): each cell's own asset id (pxl-spec
/// `ground::own` of its four at the brick's zoom).
pub const OWN_WORDS: u32 = BRICK_CELL_CNT / OWN_CELL_CNT;
/// The cells a laid word holds, an asset id each: in the cells' order
/// (`idx_of`), a 2×2 square.
pub const OWN_CELL_CNT: u32 = 4;
const _: () = assert!(BRICK_CELL_CNT.is_multiple_of(OWN_CELL_CNT));
const _: () = assert!(OWN_CELL_CNT * GroundId::BITS == u32::BITS);

/// Cell `i`'s asset id in a laid word `w`.
pub(crate) const fn own_at(w: u32, i: u32) -> u32 {
    (w >> (GroundId::BITS * (i % OWN_CELL_CNT))) & GroundId::MAX as u32
}

/// A brick's words, the part that outlives its fill, the same on the CPU and on a GPU, in
/// order: its columns, its levels, its cells as laid and the first word of its run of kept
/// spans. The clip levels' bricks are one storage buffer bound whole on a GPU, under WebGPU's floor
/// (pxl-render ray/ground.rs `GROUND_SZ`).
pub const COLUMNS_AT: u32 = 0;
pub const BOUNDS_AT: u32 = COLUMNS_AT + COLUMN_WORDS;
pub const OWN_AT: u32 = BOUNDS_AT + BOUND_WORDS;
/// The first word of its run of kept spans in the ways' pool (`RUN_BITS_AT`); 0, a word never
/// handed out, where it has none.
pub const ROOT_AT: u32 = OWN_AT + OWN_WORDS;
pub const BRICK_WORDS: u32 = ROOT_AT + 1;
/// The clip levels as one side, the CPU or the GPU, holds them: for each clip level, a
/// zoom's cells around its center, which brick each entry holds once it is present, and the
/// slots their cells are held in.
pub trait BrickSource {
    /// The slot brick `q` at `cell_zoom` is held present in, which its other reads take
    /// (`Entry::slot`); false where no clip level holds it.
    fn present(&self, cell_zoom: Zoom, q: [i32; 2]) -> (u32, bool);
    /// The brick the clip level read at `cell_zoom` wants in `entry`, and whether it is present;
    /// the inverse of `present`.
    fn brick(&self, cell_zoom: Zoom, entry: u32) -> ([i32; 2], bool);
    /// The four asset ids of cell `i` of the brick in `slot` (`asset_ids`).
    fn fold_at(&self, cell_zoom: Zoom, slot: u32, i: u32) -> u32;
    /// Column word `i` of the brick in `slot` (`COLUMN_WORDS`), `i` to
    /// `BRICK_CELL_CNT`.
    fn column(&self, cell_zoom: Zoom, slot: u32, i: u32) -> u32;
    /// Entry `at` of level `j` of the brick in `slot`: its bounds (`ground::Extent`) as bound
    /// words (`answer::bound_word`), its columns' bounds joined (`join`), level 0 being one
    /// column's.
    fn bound_mip(&self, cell_zoom: Zoom, slot: u32, j: u32, at: [u32; 2]) -> [i32; 2];
    /// Cell `i`'s own asset id as its fill laid it (`OWN_WORDS`,
    /// `own_at`).
    fn own(&self, cell_zoom: Zoom, slot: u32, i: u32) -> u32;
    /// Entry `e` of the levels over the bricks of the clip level read at `cell_zoom`
    /// (`JOINED_WORDS`, `fold`), as this side folded them over the bricks it holds; the zero
    /// entry, meaning none, where it holds no fold.
    fn joined(&self, cell_zoom: Zoom, e: u32) -> [u32; JOINED_WORDS as usize];
    /// The first brick of the clip level read at `cell_zoom`, from which its blocks of bricks
    /// are counted (`fold`), and whether the level is laid out (`ORIGIN_AT`).
    fn origin(&self, cell_zoom: Zoom) -> ([i32; 2], bool);
    /// The first word of the run of kept spans of the brick in `slot` (`ROOT_AT`), 0 for none.
    fn root(&self, cell_zoom: Zoom, slot: u32) -> u32;
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

/// The bits of a slot (`SLOT_CNT`), which a GPU and the CPU name alike.
pub const LOOKUP_SLOT_BITS: u32 = SLOT_CNT.next_power_of_two().trailing_zeros();
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
pub fn step(
    w: &impl BrickSource,
    above: Lookup,
    cell_zoom: Zoom,
    g: [i32; 2],
) -> (Lookup, [u32; 2]) {
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
/// stepped from `place` (`step`): its four asset ids, its column's word (its elevation),
/// its own asset id as laid, and where it is held; void at sea level where none holds
/// it.
pub fn cell(
    w: &impl BrickSource,
    place: Lookup,
    cell_zoom: Zoom,
    g: [i32; 2],
) -> (u32, u32, u32, Lookup) {
    let (f, at) = step(w, place, cell_zoom, g);
    if !f.here() {
        return (Sentinel::Void as u32, 0, Sentinel::Void as u32, f);
    }
    let (from, slot, i) = (f.from(), f.slot(), idx_of(at));
    (
        w.fold_at(from, slot, i),
        w.column(from, slot, i),
        w.own(from, slot, i),
        f,
    )
}

/// Column `b` at `zoom` as a reader reads it: the cell drawing it (a leaf's, finer than the
/// leaves), stepped from `place`, where the column's own cells were found (`step`), with its
/// own asset id as laid, or the wall's (`Sentinel::Unknown`) where it is the wall
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
    let asset_id = if area::is_wall(shard.at(w, (zoom, b)), zoom, b) {
        Sentinel::Unknown as u32
    } else {
        own
    };
    let (top, part) = top(zoom, column_top(column) as f32);
    let cell = Cell {
        asset_id,
        top,
        part,
        fold: asset_ids(word),
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
    let under = |q: usize| {
        let [cx, cy] = child_of([x as i32, y as i32], q);
        ground::extent_of(w.bound_mip(cell_zoom, slot, j - 1, [cx as u32, cy as u32]))
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
pub(crate) const BOUND_MIP_LAST: u32 = BRICK_MIP_LAST + ACROSS.trailing_zeros();

/// The levels over a clip level's bricks, from one level over a brick (`BRICK_MIP_LAST + 1`) to
/// the whole clip level. A node wider than a brick is bounded by the join of the bricks it
/// covers, folded where the clip levels' entries are written (`fold`) and read as one entry
/// (`bounded`).
pub const JOINED_MIP_CNT: u32 = BOUND_MIP_LAST - BRICK_MIP_LAST;
/// A clip level's entries: level `l` over a brick has `(ACROSS >> l)²`, an
/// aligned block of `2^l` bricks a side each.
pub const JOINED_ENTRY_CNT: u32 = joined_idx(JOINED_MIP_CNT + 1, [0, 0]);
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

    /// Whether a query that reads only bricks held present reads this entry
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
pub const ORIGIN_AT: u32 = SLOT_CNT;
/// The row of the clip levels' buffer the eye's clip levels' entries begin at:
/// past their entries and their first bricks.
pub const JOINED_AT: u32 = ORIGIN_AT + CLIP_LEVEL_CNT;
/// The eye's clip levels' entries, as the GPU lays them: clip level `cell_zoom −
/// FIRST_CELL_ZOOM` a run of `JOINED_ENTRY_CNT`.
pub const EYE_JOINED_WORDS: u32 = JOINED_ENTRY_CNT * JOINED_WORDS * EYE_CLIP_LEVEL_CNT;
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
const _: () = assert!(ACROSS.is_power_of_two() && JOINED_MIP_CNT < ACROSS.trailing_zeros() + 1);

/// A key's word: `v` with its sign flipped, so no brick's is 0.
const fn key_word(v: i32) -> u32 {
    v as u32 ^ 1 << 31
}

/// Entry `b` of level `l` over a brick (its Morton index there, `joined_idx`) of the clip
/// level read at `cell_zoom`, folded from the level under it. It is what a ray reads at
/// `cell_zoom` in the block of `2^l` entries a side at `b`, aligned as the entries are
/// (`ACROSS` is a multiple of the block's side): the join of its four children's joins, each
/// read once where the level under it laid it (`BrickSource::joined`; at level 1, a brick's own,
/// `brick_bound`). It is keyed by the clip level's first brick (`BrickSource::origin`), so an entry
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
            brick_bound(w, cell_zoom, q)
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
fn brick_bound(w: &impl BrickSource, cell_zoom: Zoom, q: [i32; 2]) -> Extent {
    let mut place = Lookup::NONE;
    let (b, _, here, _) = bounded(w, &mut place, cell_zoom, (cell_zoom - BRICK_MIP_LAST, q));
    if here { b } else { [ground::UNBOUNDED; 2] }
}
const _: () = assert!(
    FIRST_CELL_ZOOM >= BRICK_MIP_LAST,
    "a brick is a node at its cells' zoom less `BRICK_MIP_LAST`"
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
fn level_bound(w: &impl BrickSource, cell_zoom: Zoom, g: [i32; 2], l: u32) -> (Extent, bool) {
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
    let all = l <= JOINED_MIP_CNT
        && d[0] >= 0
        && d[1] >= 0
        && d[0] + side <= across
        && d[1] + side <= across;
    let (mip, m) = (
        if l > JOINED_MIP_CNT {
            JOINED_MIP_CNT
        } else {
            l
        },
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

/// Whether a coarser clip level's cells are read in place of some of the node `(zoom, c)`,
/// asked at the zooms from `coarsest` to `finest`, so that its bound (`bound_over`) is not
/// the asked clip levels' own: the coarsest asked does not hold the node whole, and the read
/// went on to a coarser one, or a clip level asked wants a brick under the node that it does
/// not hold present yet (`brick_bound`), or, at a zoom under the finest where the node is no
/// wider than a brick, the brick is read from a coarser clip level than that zoom's
/// (`step`). A reader that says what its answer is read from asks the nodes under such a
/// node. A node no wider than a brick at the finest zoom reads one brick there, and its
/// bound's zoom says the same.
pub fn stood_in(
    w: &impl BrickSource,
    [coarsest, finest]: [Zoom; 2],
    (zoom, c): (Zoom, [i32; 2]),
) -> bool {
    let (top, bottom) = (clip_zoom(finest), clip_zoom(coarsest));
    let first = |z: Zoom| [idx_at(zoom, c[0], z), idx_at(zoom, c[1], z)];
    let partly = bottom > zoom + BRICK_MIP_LAST
        && !level_bound(w, bottom, first(bottom), bottom - zoom - BRICK_MIP_LAST).1;
    let under = (bottom..top).any(|z| {
        let (f, _) = step(w, Lookup::NONE, z, first(z));
        z <= zoom + BRICK_MIP_LAST && f.here() && f.from() != z
    });
    partly
        || under
        || (bottom..=top).any(|z| {
            let bricks = bricks_zoom(z);
            zoom < bricks
                && w.origin(z).1
                && (0..ENTRY_CNT).any(|e| {
                    let (q, present) = w.brick(z, e);
                    let l = bricks - zoom;
                    !present
                        && wrap_idx(zoom, q[0] >> l) == wrap_idx(zoom, c[0])
                        && q[1] >> l == c[1]
                })
        })
}

/// The lowest and highest that the cells at `cell_zoom` reach under the node `(zoom, c)`.
/// For a node no wider than a brick, it reads the finest clip level holding them present,
/// stepped from `place`, which becomes where they are held (`step`): one entry of its brick's
/// levels, level 0 being the column holding a node no wider than a cell, with that clip
/// level's zoom. For a node wider than a brick, it reads what the clip level at `cell_zoom`
/// holds of it (`level_bound`), and whether it holds all of it (`bound_over` reads on where it
/// does not). It is false where no clip level holds the node, a lack.
fn bounded(
    w: &impl BrickSource,
    place: &mut Lookup,
    cell_zoom: Zoom,
    (zoom, c): (Zoom, [i32; 2]),
) -> (Extent, Zoom, bool, bool) {
    // The node's first cell at `cell_zoom`, or the cell holding it.
    let g = [idx_at(zoom, c[0], cell_zoom), idx_at(zoom, c[1], cell_zoom)];
    if cell_zoom > zoom + BRICK_MIP_LAST {
        let (b, all) = level_bound(w, cell_zoom, g, cell_zoom - zoom - BRICK_MIP_LAST);
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
