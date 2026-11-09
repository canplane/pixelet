// pxl-engine/ray.rs

//! gpu-portable: a ray's words, as the GPU's cast leaves them at the ray's drawn pixel and the
//! screen pass reads them there (pxl-render frame/shader/screen_ray.wgsl). Each word is a
//! layer of the world surface's rays. The first says what the ray stopped on, through which
//! face, at which zoom, and the share of its cube the cell keeps (`first_word`). The second is
//! how far the ray went, as f32 bits. The third holds the four palette indices the ground's
//! asset is drawn by (`ground::word`), and is zero for any other asset; in the ray steps view
//! (`eye::STEPS`) it holds the steps taken on the ray's way from the eye instead, whatever the
//! ray met: its beam ray's and its own walk's, or a sample's where it took no walk (pxl-render
//! cast.wgsl `taken_word`, queue.wgsl `finish`). The fourth says what lies around the face the
//! ray entered (`face::word`): the world's own answer about the cell, the same for every pixel
//! of it.
//!
//! A cell is painted from its own words alone, by its value, whichever source holds it.
//! Where the ray met nothing, every word but that view's third is zero; a hit's value is never
//! void.

use crate::Zoom;

/// A ray's words, and the index of the word that says what lies around its face.
pub const WORDS: u32 = 4;
pub const FACES: u32 = 3;

/// The first word's fields, from the low bits: the value, a catalog value whichever source
/// holds it; the face the ray entered through, 2·axis plus one where its outward normal
/// points down the axis; the zoom of the cell it stopped on, in six bits; and the eighths of
/// its cube the cell keeps, 0 for all eight (`answer::fill_eighths`), so the screen pass draws
/// the box the cell is (`answer::side`).
pub use crate::answer::VAL_BITS;
pub const FACE_BITS: u32 = 3;
use crate::answer;
pub use crate::answer::FILL_BITS;
use crate::cast::STEPS_CAP;
use crate::count;
use crate::FINEST_ZOOM;
pub use crate::ZOOM_BITS;

pub const FACE_SHIFT: u32 = VAL_BITS;
pub const ZOOM_SHIFT: u32 = FACE_SHIFT + FACE_BITS;
pub const FILL_SHIFT: u32 = ZOOM_SHIFT + ZOOM_BITS;
const _: () = assert!(FILL_SHIFT + FILL_BITS < 32);

/// The shape of a pass over a list walked apart, or over the walked bits: `MISS_LANES` lanes
/// a workgroup and `MISS_ACROSS` workgroups a row (pxl-render list.wgsl `list_place`).
pub const MISS_LANES: u32 = 64;
pub const MISS_ACROSS: u32 = 64;

/// What the pixels' cast counts, and the light slots' cast alike (pxl-render visible.wgsl
/// `MissCount`): first the holes it listed (`MISS_ARGS_CNT`, `list_hole`), then, while the
/// `ray_cnts` setting is on, what its pixels or slots took (`MISS_ARGS_CNTS`: the
/// `count::RAYS` counters from `count::VOXEL`). Each frame starts from `MISS_ARGS_EMPTY`.
pub const MISS_ARGS_CNT: u32 = 0;
pub const MISS_ARGS_CNTS: u32 = 1;
pub const MISS_ARGS_WORDS: usize = (MISS_ARGS_CNTS + count::RAYS) as usize;
pub const MISS_ARGS_EMPTY: [u32; MISS_ARGS_WORDS] = [0; MISS_ARGS_WORDS];

/// The eye's holes, past the camera grid's words in the beam rays' buffer
/// (`beam::Grid::walked_at`). First come the walked bits, one bit a drawn pixel, marking the
/// pixels the last cast walked; only a cast writes them, and the light's receivers read them
/// (pxl-render light_mark.wgsl). The pixels lie in squares of `CAST_SIDE`, each square one
/// workgroup of the pixels' cast (pxl-render cast.wgsl `cast_cs`) and `SQUARE_WORDS` words,
/// with the squares row by row; a pixel's bit within its square is row-major (`hole_bit`).
/// Then come the holes the cast listed: their count, then one word a pixel, with the row in
/// the high bits and the column in the low (`holes_at`; pxl-render queue.wgsl). Last is the
/// pool their rays are parked in between a run's walks (`pool_at`).
pub const SQUARE_WORDS: u32 = CAST_SIDE * CAST_SIDE / 32;
const _: () = assert!(
    (CAST_SIDE * CAST_SIDE).is_multiple_of(32),
    "a square of pixels is whole words"
);

/// The squares across and down of a target `size` drawn pixels each way.
pub const fn squares(size: [u32; 2]) -> [u32; 2] {
    [size[0].div_ceil(CAST_SIDE), size[1].div_ceil(CAST_SIDE)]
}

/// The words of the walked bits for a target of `size`.
pub const fn walked_words(size: [u32; 2]) -> u32 {
    let [across, down] = squares(size);
    across * down * SQUARE_WORDS
}

/// Pixel `p`'s word within one set of hole bits of a target of `size`, and its bit in that
/// word.
pub const fn hole_bit(p: [u32; 2], size: [u32; 2]) -> (u32, u32) {
    let across = squares(size)[0];
    let square = (p[1] / CAST_SIDE) * across + p[0] / CAST_SIDE;
    let bit = (p[1] % CAST_SIDE) * CAST_SIDE + p[0] % CAST_SIDE;
    (square * SQUARE_WORDS + bit / 32, bit % 32)
}

/// Where the listed holes' count lies, counted from the walked bits' first word; the holes
/// follow it.
pub const fn holes_at(size: [u32; 2]) -> u32 {
    walked_words(size)
}

/// The room of a pool: the most rays a run keeps parked between its walks. A ray past it
/// ends as a lack where it stands.
pub const POOL_CAP: u32 = 1 << 18;

/// A parked ray as a pool keeps it: its state (`STATE_WORDS`), then its hole (`PARK_HOLE`),
/// with the row in the high bits and the column in the low.
pub const PARK_HOLE: u32 = STATE_WORDS;
pub const PARK_WORDS: u32 = STATE_WORDS + 1;

/// A pool: room for `POOL_CAP` parked rays.
pub const POOL_WORDS: u32 = POOL_CAP * PARK_WORDS;

/// Where the eye's pool lies, counted from the walked bits' first word.
pub const fn pool_at(size: [u32; 2]) -> u32 {
    holes_at(size) + 1 + size[0] * size[1]
}

/// The words of the eye's holes for a target of `size`: the walked bits, the list and the
/// pool.
pub const fn holes_words(size: [u32; 2]) -> u32 {
    pool_at(size) + POOL_WORDS
}

/// The dispatch and count of a list walked apart as it starts (pxl-render list.wgsl
/// `enlist`): `MISS_ACROSS` workgroups a row, no rows until a hole is listed, and a count of
/// none.
pub const LEFT_EMPTY: [u32; 4] = [MISS_ACROSS, 0, 1, 0];

/// A run's walks (pxl-render queue_walk.wgsl, queue_ask.wgsl, queue_last.wgsl) are a fixed
/// dispatch of `QUEUE_GROUPS` workgroups of `QUEUE_LANES` lanes. Each lane takes the next ray
/// when its own ends or is parked, and walks it `QUEUE_STEPS` steps at a time. The passes run
/// in this order: the first walk, over the node cache alone; the asks, one lane for each node
/// a walk was parked at, each node once; the second walk, over the rays the first parked; the
/// asks again; and the last walk, which asks the world where no slot keeps a node. Every ray
/// ends within its run.
pub const QUEUE_LANES: u32 = 64;
pub const QUEUE_STEPS: u32 = 16;
pub const QUEUE_GROUPS: u32 = 256;

/// The most nodes a frame asks of the world, all runs together (`node_cache::ASKED`). What a
/// frame defers is only detail: a ray that may ask no more ends as a lack at the deepest node
/// held.
pub const ASKS_A_FRAME: u32 = 1 << 21;

/// The stops an observer's list holds (pxl-render queue.wgsl `hit_in_cap`,
/// `beam::Grid::hits_at`, `light::hits_at`). A ray past it ends with no stop kept for the
/// record, and its pixel shows a box's face word (`face::BOX`) instead of its face's own.
pub const HITS_A_FRAME: u32 = 1 << 18;

/// The queues' words, all in one buffer (pxl-render queue_walk.wgsl `queue`,
/// queue_ask.wgsl). First comes a head for each run's queue (`QUEUES`), each
/// `QUEUE_HEADER_STRIDE` words after the last, from which the run's passes bind their view
/// (`queue_at`). Then come the asks' records, one region that every queue's view reaches
/// (`queue_records_at`). One region is enough because the runs of a frame, and the two walks
/// of a run, go one after another, and no pass reads a record past the asks that follow the
/// walk that listed it.
///
/// A head holds the next hole the first walk takes (`QUEUE_NEXT`) and the next the second
/// takes (`QUEUE_NEXT_PARKED`); the nodes each walk was parked at, listed for the asks
/// (`QUEUE_ASKS`, `QUEUE_ASKS_PARKED`); the stops the two walks listed, laid out as
/// `node_cache::HITS_ARGS` (`QUEUE_HITS`); and the rays its walks parked (`QUEUE_PARKED`). Each
/// run starts from `QUEUE_EMPTY`.
pub const QUEUE_NEXT: u32 = 0;
pub const QUEUE_NEXT_PARKED: u32 = 1;
pub const QUEUE_ASKS: u32 = 2;
pub const QUEUE_ASKS_PARKED: u32 = 3;
pub const QUEUE_HITS: u32 = 4;
pub const QUEUE_PARKED: u32 = 8;
pub const QUEUE_HEADER_WORDS: u32 = 9;
/// The runs' queues, a head each: the eye's beam rays, its samples and its holes, and the
/// same three for the light.
pub const QUEUE_EYE_CORNERS: u32 = 0;
pub const QUEUE_EYE_SAMPLES: u32 = 1;
pub const QUEUE_EYE_HOLES: u32 = 2;
pub const QUEUE_LIGHT_CORNERS: u32 = 3;
pub const QUEUE_LIGHT_SAMPLES: u32 = 4;
pub const QUEUE_LIGHT_HOLES: u32 = 5;
pub const QUEUES: u32 = 6;
/// The words from one head to the next: the alignment a storage buffer's view is bound at
/// (`STORAGE_ALIGN_WORDS`).
pub const QUEUE_HEADER_STRIDE: u32 = crate::STORAGE_ALIGN_WORDS;
const _: () = assert!(
    QUEUE_HEADER_WORDS <= QUEUE_HEADER_STRIDE,
    "a header fits its stride"
);

/// The first word of queue `queue`'s head, which is also the first word of its view.
pub const fn queue_at(queue: u32) -> u32 {
    queue * QUEUE_HEADER_STRIDE
}

/// The first word of the asks' records, counted from the first word of queue `queue`'s view:
/// past every head.
pub const fn queue_records_at(queue: u32) -> u32 {
    queue_at(QUEUES) - queue_at(queue)
}

/// An ask's record: the node's zoom, the three words of its index, the zooms it is asked at
/// (the far zoom, then the near one `ZOOM_BITS` above it), whether it is asked from the top,
/// and what was handed to its parent.
pub const QUEUE_RECORD_WORDS: u32 = 9;
/// The most nodes one walk lists for the asks after it. A ray parked at a node listed past
/// it is asked by the last walk instead.
pub const QUEUE_RECORDS_CAP: u32 = 1 << 17;
pub const QUEUE_WORDS: u32 = queue_at(QUEUES) + QUEUE_RECORDS_CAP * QUEUE_RECORD_WORDS;
pub const QUEUE_EMPTY: [u32; QUEUE_HEADER_WORDS as usize] = [0, 0, 0, 0, 0, 1, 1, 0, 0];

/// A parked ray's state as the queue keeps it, the same for every kind of item (pxl-render
/// parked.wgsl): its distance as f32 bits, its node's corner, and its zoom, roots and steps
/// taken, packed under `PARKED`, the bit that says the ray is parked there.
pub const STATE_WORDS: u32 = 5;
pub const PARKED: u32 = 1 << 31;
const _: () = assert!(
    STEPS_CAP < 1 << 13 && FINEST_ZOOM < 1 << 6,
    "a state's zoom, roots and steps fit under `PARKED`: 6, 9 and 13 bits"
);

/// The side of the square of pixels one workgroup of the pixels' cast takes (pxl-render
/// cast.wgsl `cast_cs`).
pub const CAST_SIDE: u32 = 8;

/// The face a ray going `dir` enters across `axis`: 2·axis, plus one where the ray goes up
/// the axis, since that face's outward normal points down it. A ray that does not move along
/// the axis counts as going down it.
pub fn face(dir: [f32; 3], axis: u32) -> u32 {
    2 * axis + u32::from(crate::axis(dir, axis) > 0.0)
}

/// The first word of a hit on `asset`, an asset's word (`answer`): its value and the share
/// of the cube it keeps, with the face and the zoom.
pub const fn first_word(asset: u32, face: u32, zoom: Zoom) -> u32 {
    let fill = asset >> answer::FILL_SHIFT & ((1 << FILL_BITS) - 1);
    answer::val(asset) | face << FACE_SHIFT | zoom << ZOOM_SHIFT | fill << FILL_SHIFT
}
