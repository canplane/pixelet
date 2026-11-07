// pxl-engine/ray.rs

//! gpu-portable: a ray's words as the GPU's cast leaves them at its drawn
//! pixel and the screen pass reads them there (pxl-render
//! frame/shader/screen_ray.wgsl), a layer of the world surface's rays a
//! word: what it stopped on, through which face, at which zoom and the
//! share of its cube it keeps (`first_word`); how far, f32 bits; the four palette indices the ground's asset is
//! drawn by (`ground::word`, none for any other); and what lies about the
//! face it entered (`face::word`), the world's own answer about the cell, the
//! same for every pixel of it. A cell is painted from these, its own, alone,
//! by its value whichever source holds it. Where it met nothing, every word is
//! zero: a hit's value is never void.

use crate::Zoom;

/// A ray's words, and the word of what lies about its face.
pub const WORDS: u32 = 4;
pub const FACES: u32 = 3;

/// The first word's fields: the value, one of the catalog's whichever
/// source holds it; the face it entered through, 2·axis plus one where its
/// outward normal points down the axis; the zoom of the cell it stopped on,
/// six bits; the eighths of its cube the cell keeps, 0 for all eight
/// (`answer::fill_eighths`), so the screen pass draws the box it is
/// (`answer::side`).
pub use crate::answer::VAL_BITS;
pub const FACE_BITS: u32 = 3;
use crate::answer;
pub use crate::answer::FILL_BITS;
use crate::cast::STEPS_MOST;
use crate::count;
use crate::FINEST_ZOOM;
pub use crate::ZOOM_BITS;

pub const FACE_SHIFT: u32 = VAL_BITS;
pub const ZOOM_SHIFT: u32 = FACE_SHIFT + FACE_BITS;
pub const FILL_SHIFT: u32 = ZOOM_SHIFT + ZOOM_BITS;
const _: () = assert!(FILL_SHIFT + FILL_BITS < 32);

/// A pass over a list walked apart, or over the walked bits: `MISS_LANES` a workgroup,
/// `MISS_ACROSS` workgroups a row (pxl-render list.wgsl `list_place`).
pub const MISS_LANES: u32 = 64;
pub const MISS_ACROSS: u32 = 64;

/// What the pixels' cast counts, the light's slots' alike (pxl-render visible.wgsl `MissArgs`):
/// the holes it listed (`MISS_ARGS_CNT`, `list_hole`); then what its pixels, or the light's
/// slots, took where the `ray_counts` setting asks (`count::RAYS` lanes from `count::REUSED`,
/// `MISS_ARGS_COUNT`). Each frame starts with none (`MISS_ARGS_EMPTY`).
pub const MISS_ARGS_CNT: u32 = 0;
pub const MISS_ARGS_COUNT: u32 = 1;
pub const MISS_ARGS_WORDS: usize = (MISS_ARGS_COUNT + count::RAYS) as usize;
pub const MISS_ARGS_EMPTY: [u32; MISS_ARGS_WORDS] = [0; MISS_ARGS_WORDS];

/// The eye's holes, past the camera grid's words in the beam rays' buffer
/// (`beam::Grid::walked_at`). First the walked bits, a bit a drawn pixel, the pixels the last
/// cast marked, which only a cast writes and the light's receivers read (light_mark.wgsl): the
/// pixels lie in squares of `CAST_SIDE`, one workgroup of the pixels' cast each (pxl-render
/// cast.wgsl `cast_cs`), `SQUARE_WORDS` words a square, the squares row by row, and a pixel's
/// bit within its square is its row over its column (`hole_bit`). Then the holes the cast
/// listed, how many then a pixel each, its column under its row (`holes_at`; pxl-render
/// queue.wgsl); then the pool their rays are parked in between a run's walks (`pool_at`).
pub const SQUARE_WORDS: u32 = CAST_SIDE * CAST_SIDE / 32;
const _: () = assert!(
    (CAST_SIDE * CAST_SIDE).is_multiple_of(32),
    "a square of pixels is whole words"
);

/// The squares of a target `size` drawn pixels each way, across and down.
pub const fn squares(size: [u32; 2]) -> [u32; 2] {
    [size[0].div_ceil(CAST_SIDE), size[1].div_ceil(CAST_SIDE)]
}

/// The walked bits' words of a target `size`.
pub const fn walked_words(size: [u32; 2]) -> u32 {
    let [across, down] = squares(size);
    across * down * SQUARE_WORDS
}

/// Pixel `p`'s word among one set of the holes' bits of a target `size`, and its bit in it.
pub const fn hole_bit(p: [u32; 2], size: [u32; 2]) -> (u32, u32) {
    let across = squares(size)[0];
    let square = (p[1] / CAST_SIDE) * across + p[0] / CAST_SIDE;
    let bit = (p[1] % CAST_SIDE) * CAST_SIDE + p[0] % CAST_SIDE;
    (square * SQUARE_WORDS + bit / 32, bit % 32)
}

/// The listed holes' count, from the walked bits' first word, their entries after it.
pub const fn holes_at(size: [u32; 2]) -> u32 {
    walked_words(size)
}

/// The rays a run keeps parked between its walks, a pool's room. A ray past it ends as a lack
/// where it stands.
pub const POOL_CAP: u32 = 1 << 18;

/// A parked ray as a pool keeps it: its state (`STATE_WORDS`), then its pixel, its column under
/// its row (`PARK_PX`).
pub const PARK_PX: u32 = STATE_WORDS;
pub const PARK_WORDS: u32 = STATE_WORDS + 1;

/// A pool: how many rays its run parked in it (`POOL_CNT`), copied in from the queue's words
/// (`QUEUE_PARKED`) once the run's walks over the node cache are done, then `POOL_CAP` parked
/// rays from `POOL_STATES`.
pub const POOL_CNT: u32 = 0;
pub const POOL_STATES: u32 = 1;
pub const POOL_WORDS: u32 = POOL_STATES + POOL_CAP * PARK_WORDS;

/// The eye's pool, from the walked bits' first word.
pub const fn pool_at(size: [u32; 2]) -> u32 {
    holes_at(size) + 1 + size[0] * size[1]
}

/// The eye's holes' words of a target `size`: its walked bits, its list and its pool.
pub const fn holes_words(size: [u32; 2]) -> u32 {
    pool_at(size) + POOL_WORDS
}

/// A list walked apart's dispatch and count as it starts (pxl-render list.wgsl `enlist`):
/// `MISS_ACROSS` workgroups a row, no row until one is listed, then none counted.
pub const LEFT_EMPTY: [u32; 4] = [MISS_ACROSS, 0, 1, 0];

/// A run's walks (pxl-render queue_walk.wgsl, queue_ask.wgsl, queue_last.wgsl), a fixed dispatch
/// of `QUEUE_GROUPS` workgroups of `QUEUE_LANES`, each lane taking the next ray when its own ends
/// or is parked and walking it `QUEUE_STEPS` steps at a time. In turn: the first walk over the
/// node cache alone; the asks, a lane a node a walk was parked at, once a node; the second walk
/// over the rays the first parked; the asks again; and the last walk, which asks the world where
/// no slot keeps a node. Every ray ends in its run.
pub const QUEUE_LANES: u32 = 64;
pub const QUEUE_STEPS: u32 = 16;
pub const QUEUE_GROUPS: u32 = 256;

/// The nodes a frame asks of the world at most, every run's together (`node_cache::ASKED`): what
/// a frame defers is detail. A ray that may ask no more ends as a lack at the deepest node held.
pub const ASKS_A_FRAME: u32 = 1 << 21;

/// The stops a list holds, an observer's (pxl-render queue.wgsl `stop_room`,
/// `beam::Grid::hits_at`, `light::hits_at`): a ray past it ends with no stop kept for the
/// record, its pixel showing no face's word but a box's (pxl-engine `face::BOX`).
pub const STOPS_A_FRAME: u32 = 1 << 18;

/// The queues' words, one buffer of them (pxl-render queue_walk.wgsl `queue`, queue_ask.wgsl):
/// a head for each run's queue (`QUEUES`), each `QUEUE_HEAD_STRIDE` words from the last, which
/// its run's passes bind their view from (`queue_at`); then the asks' records, one region every
/// queue's view reaches (`queue_records_at`), as the runs of a frame and the two walks of a run
/// go one after another and none reads a record past the asks that follow the walk that listed
/// it.
///
/// A head: the next hole the first walk takes (`QUEUE_TAKEN`) and the next the second takes
/// (`QUEUE_TAKEN_AGAIN`); the nodes each walk was parked at, listed for the asks (`QUEUE_ASKS`,
/// `QUEUE_ASKS_AGAIN`); the stops the two walks listed, as `node_cache::HITS_ARGS` lays them
/// (`QUEUE_HITS`); and the rays its walks parked in the pool (`QUEUE_PARKED`, `POOL_CNT`). Each
/// run's start is `QUEUE_EMPTY`.
pub const QUEUE_TAKEN: u32 = 0;
pub const QUEUE_TAKEN_AGAIN: u32 = 1;
pub const QUEUE_ASKS: u32 = 2;
pub const QUEUE_ASKS_AGAIN: u32 = 3;
pub const QUEUE_HITS: u32 = 4;
pub const QUEUE_PARKED: u32 = 8;
pub const QUEUE_HEAD_WORDS: u32 = 9;
/// The runs' queues, a head each: the eye's beam rays', its samples' and its holes'; the
/// light's alike.
pub const QUEUE_EYE_CORNERS: u32 = 0;
pub const QUEUE_EYE_SAMPLES: u32 = 1;
pub const QUEUE_EYE_HOLES: u32 = 2;
pub const QUEUE_LIGHT_CORNERS: u32 = 3;
pub const QUEUE_LIGHT_SAMPLES: u32 = 4;
pub const QUEUE_LIGHT_HOLES: u32 = 5;
pub const QUEUES: u32 = 6;
/// The words from one head to the next: the offset a storage buffer's view is bound at
/// (`STORAGE_ALIGN_WORDS`).
pub const QUEUE_HEAD_STRIDE: u32 = crate::STORAGE_ALIGN_WORDS;
const _: () = assert!(
    QUEUE_HEAD_WORDS <= QUEUE_HEAD_STRIDE,
    "a head fits its stride"
);

/// The first word of queue `queue`'s head, its view's first.
pub const fn queue_at(queue: u32) -> u32 {
    queue * QUEUE_HEAD_STRIDE
}

/// The first word of the asks' records from queue `queue`'s view's first: past every head.
pub const fn queue_records_at(queue: u32) -> u32 {
    queue_at(QUEUES) - queue_at(queue)
}

/// An ask's record: the node's zoom, its index's three words, the zooms it is asked at, far then
/// near over `ZOOM_BITS`, whether it is asked from the top, and what was handed its parent.
pub const QUEUE_RECORD_WORDS: u32 = 10;
/// The nodes one walk lists for the asks after it at most. A ray parked at a node listed past it
/// is asked by the last walk instead.
pub const QUEUE_RECORDS_ROOM: u32 = 1 << 17;
pub const QUEUE_WORDS: u32 = queue_at(QUEUES) + QUEUE_RECORDS_ROOM * QUEUE_RECORD_WORDS;
pub const QUEUE_EMPTY: [u32; QUEUE_HEAD_WORDS as usize] = [0, 0, 0, 0, 0, 1, 1, 0, 0];

/// A parked ray's state, the queue's (pxl-render parked.wgsl), every item kind's alike: its
/// distance's bits, its node's corner, and its zoom, roots and steps taken packed under `PARKED`,
/// which says the ray is parked there.
pub const STATE_WORDS: u32 = 5;
pub const PARKED: u32 = 1 << 31;
const _: () = assert!(
    STEPS_MOST < 1 << 13 && FINEST_ZOOM < 1 << 6,
    "a state's zoom, roots and steps fit under `PARKED`: 6, 9 and 13 bits"
);

/// The side of the pixels' pass's workgroup, a square of pixels (pxl-render cast.wgsl
/// `cast_cs`).
pub const CAST_SIDE: u32 = 8;

/// The face a ray going `dir` enters across `axis`: 2·axis, and one more where it goes up
/// that axis, the face's outward normal pointing down it; a ray that does not move along the
/// axis enters it as one going down.
pub fn face(dir: [f32; 3], axis: u32) -> u32 {
    2 * axis + u32::from(crate::axis(dir, axis) > 0.0)
}

/// The first word of a hit at `asset`, an asset's word (`answer`): its
/// value and the share it keeps, the face and the zoom.
pub const fn first_word(asset: u32, face: u32, zoom: Zoom) -> u32 {
    let fill = asset >> answer::FILL_SHIFT & ((1 << FILL_BITS) - 1);
    answer::val(asset) | face << FACE_SHIFT | zoom << ZOOM_SHIFT | fill << FILL_SHIFT
}
