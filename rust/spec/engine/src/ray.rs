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

/// The walk again's dispatch, as the reading walk counts what it marks
/// (pxl-render cast.wgsl `cast_cs`): over the marked pixels listed
/// (`WALKED_AT`), `MISS_LANES` a workgroup, `MISS_ACROSS`
/// workgroups a row and a row each `MISS_LANES * MISS_ACROSS` listed
/// (`MISS_ARGS_LIST`); the count listed (`MISS_ARGS_CNT`); then what its pixels, or the
/// light's slots, counted where the `ray_counts` setting asks (`count::RAYS`
/// lanes from `count::REUSED`, `MISS_ARGS_COUNT`). Each frame's start
/// (`MISS_ARGS_EMPTY`) dispatches none and counts none. The light's slots' list is laid alike.
pub const MISS_LANES: u32 = 64;
pub const MISS_ACROSS: u32 = 64;
pub const MISS_ARGS_LIST: u32 = 0;
pub const MISS_ARGS_CNT: u32 = 3;
pub const MISS_ARGS_COUNT: u32 = 4;
pub const MISS_ARGS_WORDS: usize = (MISS_ARGS_COUNT + count::RAYS) as usize;
pub const MISS_ARGS_EMPTY: [u32; MISS_ARGS_WORDS] = [MISS_ACROSS, 0, 1, 0, 0, 0, 0];

/// The eye's holes, past the camera grid's words in the beam rays' buffer
/// (`beam::Grid::walked_at`): a bit a drawn pixel, and no other word any pixel takes. The
/// pixels lie in squares of `CAST_SIDE`, one workgroup of the pixels' cast each (pxl-render
/// cast.wgsl `cast_cs`), `SQUARE_WORDS` words a square, the squares row by row, and a pixel's bit
/// within its square is its row over its column (`hole_bit`). First the walked bits, the pixels
/// the last cast marked, which only a cast writes and the light's receivers read
/// (light_mark.wgsl); then the owed bits (`owed_bits_at`), the holes no ray of the queue holds:
/// a cast's run takes them as the walked bits are, and the queue's first walk clears each as
/// it takes it (pxl-render queue.wgsl); then the word saying what the queue's run takes
/// (`from_at`), and the two pools its rays are parked in (`pool_at`).
pub const SQUARE_WORDS: u32 = CAST_SIDE * CAST_SIDE / 32;
const _: () = assert!(
    (CAST_SIDE * CAST_SIDE).is_multiple_of(32),
    "a square of pixels is whole words"
);

/// The squares of a target `size` drawn pixels each way, across and down.
pub const fn squares(size: [u32; 2]) -> [u32; 2] {
    [size[0].div_ceil(CAST_SIDE), size[1].div_ceil(CAST_SIDE)]
}

/// The words of one set of the holes' bits of a target `size`.
pub const fn hole_words(size: [u32; 2]) -> u32 {
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

/// The owed bits' first word, from the walked bits'.
pub const fn owed_bits_at(size: [u32; 2]) -> u32 {
    hole_words(size)
}

/// The word saying what the eye's queue takes this run, from the walked bits' first word, the
/// host's each frame it runs: a cast's run, the owed bits as its cast set them and no ray
/// parked (`FROM_WALKED`); a run alone, the owed bits the runs before left and the rays the run
/// before parked, in pool `from - FROM_OWED` (`FROM_OWED`). The run parks its rays in the other
/// pool (`owed_into`).
pub const fn from_at(size: [u32; 2]) -> u32 {
    owed_bits_at(size) + hole_words(size)
}
pub const FROM_WALKED: u32 = 0;
pub const FROM_OWED: u32 = 1;

/// The pool a run parks its rays in, where it takes `from`: the one it does not take.
pub const fn owed_into(from: u32) -> u32 {
    if from == FROM_OWED {
        1
    } else {
        0
    }
}

/// The eye's rays a run keeps parked, a pool's room: the queue's lanes, each parking a ray
/// `PARK_ROUNDS` times over. The first walk takes another hole only while every lane could still
/// park its ray within the room (pxl-render queue.wgsl `takes_more`), and the frame's budget
/// (`VISITS_A_FRAME`) bounds the walks the parked rays take. A hole no lane took keeps its owed
/// bit, and its ray starts from its cell's start when a later run takes it.
pub const PARK_ROUNDS: u32 = 16;
pub const PARKED_ROOM: u32 = QUEUE_GROUPS * QUEUE_LANES * PARK_ROUNDS;
const _: () = assert!(
    PARKED_ROOM > QUEUE_GROUPS * QUEUE_LANES,
    "a pool holds a ray of every lane"
);

/// A parked ray as a pool keeps it: its state (`STATE_WORDS`), then its pixel, its column under
/// its row (`PARK_PX`).
pub const PARK_PX: u32 = STATE_WORDS;
pub const PARK_WORDS: u32 = STATE_WORDS + 1;

/// A pool: how many rays its run parked in it (`POOL_CNT`), copied in from the queue's words
/// (`QUEUE_PARKED`) once the run's walks over the node cache are done, then `PARKED_ROOM` parked
/// rays from `POOL_STATES`.
pub const POOL_CNT: u32 = 0;
pub const POOL_STATES: u32 = 1;
pub const POOL_WORDS: u32 = POOL_STATES + PARKED_ROOM * PARK_WORDS;

/// Pool `k`'s first word, from the walked bits' first word.
pub const fn pool_at(size: [u32; 2], k: u32) -> u32 {
    from_at(size) + 1 + k * POOL_WORDS
}

/// The eye's holes' words of a target `size`: its two sets of bits, a bit each a pixel, then
/// the word of what its run takes and its two pools, which no pixel sizes.
pub const fn holes_words(size: [u32; 2]) -> u32 {
    pool_at(size, 2)
}

/// A list's dispatch and count as a frame starts (pxl-render list.wgsl `enlist`): laid as
/// `MISS_ARGS_LIST`'s, no row until one is listed, then none counted. Every list raises the
/// same row word.
pub const LEFT_EMPTY: [u32; 4] = [MISS_ACROSS, 0, 1, 0];
const _: () = assert!(
    MISS_ARGS_LIST == 0,
    "a list's rows are its dispatch's second word"
);

/// The holes' queue (pxl-render queue.wgsl): the pixels the eye's cast and the slots the
/// light's leave unanswered by any voxel, each observer's walked off one queue by lanes that
/// each take the next ray when theirs ends or is parked, a fixed dispatch of
/// `QUEUE_GROUPS` workgroups of `QUEUE_LANES` whatever the count, which it reads where the
/// walks wrote it: no indirect call, so no command buffer is cut for it. A lane walks its ray
/// `QUEUE_STEPS` steps at a time and keeps it, in its registers, between them.
///
/// In turn: the first walk, over the node cache alone, over the observer's holes, a hole parked
/// before going on from its state (a light's slot of an owed tile, `light::READ_OWED`; an
/// eye's ray the run before parked, `PARKED_ROOM`); the asks, a lane a node a walk was
/// parked at, once a node; the second walk, over the node cache alone, over the rays the first
/// parked, from the state each was parked in; the asks again; and the last walk, which asks
/// the world where no slot keeps a node, over the rays the second parked. Every run counts its
/// work against the frame's budget (`VISITS_A_FRAME`, `ASKS_A_FRAME`) up to its kind's caps
/// (`caps`): a hole the budget does not reach stays parked and is owed
/// (`node_cache::OWED_AT`), a light's slot's tile to the light's next claim, an eye's pixel to
/// the next frame's queue while the rays stand, as its owed bit or its parked ray.
pub const QUEUE_LANES: u32 = 64;
pub const QUEUE_GROUPS: u32 = 256;
pub const QUEUE_STEPS: u32 = 16;

/// The work a frame's queues may do, every run's together (pxl-render queue_walk.wgsl,
/// queue_ask.wgsl, queue_last.wgsl): the nodes their walks visit, and the nodes they ask of the
/// world, the beam rays' among them. A ray the budget does not reach is parked and owed
/// (`node_cache::OWED_AT`). To be set from the S23's hot drive; the asks the most a whole frame
/// of the kernel's harness (pxl-kernel tests/frame) needs to rest within its bound.
pub const VISITS_A_FRAME: u32 = 1 << 23;
pub const ASKS_A_FRAME: u32 = 1 << 17;

/// The most a walk's lanes visit past a cap: every lane of the dispatch saw the visits under
/// it and walked one run of steps more. Asks are counted one at a time and pass no cap.
pub const OVERRUN: u32 = QUEUE_GROUPS * QUEUE_LANES * QUEUE_STEPS;

/// The least share of the budget every run of a frame does, the same for each kind
/// (`count::RUNS`), so no run starves another: the visits' and the asks'. To be set from the
/// S23.
pub const VISITS_SHARE: u32 = (VISITS_A_FRAME - (count::RUNS - 1) * OVERRUN) / count::RUNS;
pub const ASKS_SHARE: u32 = ASKS_A_FRAME / count::RUNS;

/// The caps a run's work counts to, the frame's work so far included, the visits' and the
/// asks' (`node_cache::caps_at`), where `after` runs of the frame follow it: the budget short
/// of each later run's share and of what its lanes may overrun into it. So every run does at
/// least its share, the first too.
pub const fn caps(after: u32) -> [u32; 2] {
    [
        VISITS_A_FRAME - after * (VISITS_SHARE + OVERRUN),
        ASKS_A_FRAME - after * ASKS_SHARE,
    ]
}
const _: () = assert!(
    caps(count::RUNS - 1)[0] >= VISITS_SHARE && caps(count::RUNS - 1)[1] >= ASKS_SHARE,
    "the first run's cap holds its share"
);

/// The queues' words, one buffer of them (pxl-render queue_walk.wgsl `queue`, queue_ask.wgsl):
/// a head for each run's queue (`QUEUES`), each `QUEUE_HEAD_STRIDE` words from the last, which
/// its run's passes bind their view from (`queue_at`); then the asks' records, one region every
/// queue's view reaches (`queue_records_at`), as the runs of a frame and the two walks of a run
/// go one after another and none reads a record past the asks that follow the walk that listed
/// it.
///
/// A head: the next hole the first walk takes (`QUEUE_TAKEN`) and the next the second takes
/// (`QUEUE_TAKEN_AGAIN`); the nodes each walk was parked at, listed for the asks (`QUEUE_ASKS`,
/// `QUEUE_ASKS_AGAIN`); the nodes the walk over the node cache last run visited, which it counts
/// against its budget beside the frame's (`node_cache::VISITED`) and the asks after it fold into
/// the frame's (`QUEUE_VISITED`); the stops the two walks listed, as `node_cache::HITS_ARGS`
/// lays them (`QUEUE_HITS`); and the eye's holes' next word of owed bits the first walk takes
/// after the rays the run before parked (`QUEUE_SQUARES`), and the rays its walks parked in the
/// run's pool (`QUEUE_PARKED`, `POOL_CNT`). Each run's start is `QUEUE_EMPTY`.
pub const QUEUE_TAKEN: u32 = 0;
pub const QUEUE_TAKEN_AGAIN: u32 = 1;
pub const QUEUE_ASKS: u32 = 2;
pub const QUEUE_ASKS_AGAIN: u32 = 3;
pub const QUEUE_VISITED: u32 = 4;
pub const QUEUE_HITS: u32 = 5;
pub const QUEUE_SQUARES: u32 = 9;
pub const QUEUE_PARKED: u32 = 10;
pub const QUEUE_HEAD_WORDS: u32 = 11;
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
/// The nodes one walk lists for the asks after it at most: the frame's asks
/// (`ASKS_A_FRAME`), the most its run's caps let them ask (`caps`). A ray parked at a node
/// listed past it is asked by the last walk instead.
pub const QUEUE_RECORDS_ROOM: u32 = ASKS_A_FRAME;
pub const QUEUE_WORDS: u32 = queue_at(QUEUES) + QUEUE_RECORDS_ROOM * QUEUE_RECORD_WORDS;
pub const QUEUE_EMPTY: [u32; QUEUE_HEAD_WORDS as usize] = [0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0];

/// A parked ray's state, the queue's (pxl-render queue.wgsl `park`), every item kind's alike:
/// its distance's bits, its node's corner, and its zoom, roots and steps taken packed under
/// `PARKED`, which says the ray is parked there. The eye's holes keep theirs in a pool, its pixel
/// beside it (`PARK_WORDS`); the light's holes in its region (`region_words`), a state each of
/// the slots of the tiles a cast claims (light.wgsl, a thread a slot), by its tile's place and
/// its slot within the tile, a cast's and the one before's (`light::region_words`).
///
/// A light's slot whose ray is not parked in a tile its claim carries over from the cast before
/// (pxl-render light_propagate.wgsl `claim`, `light::HEAD_CARRIES`) holds `KEPT` alone: that
/// cast answered it, by its visible voxel, the sky or a walk that ended, over the light's
/// content as it still stands, so its word stands and no walk takes it again (light.wgsl).
pub const STATE_WORDS: u32 = 5;
pub const PARKED: u32 = 1 << 31;
pub const KEPT: u32 = 1 << 30;
pub const fn region_words(room: u32) -> u32 {
    room * STATE_WORDS
}
const _: () = assert!(
    STEPS_MOST < 1 << 13 && FINEST_ZOOM < 1 << 6,
    "a state's zoom, roots and steps fit under `KEPT`: 6, 9 and 13 bits"
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Every run of a frame, each at its cap and past it by the most its lanes overrun: each
    /// still has its share of the visits and of the asks to do; one alone has the whole budget.
    #[test]
    fn each_run_keeps_its_share() {
        for runs in 1..=count::RUNS {
            let mut spent = [0, 0];
            for k in 0..runs {
                let [visits, asks] = caps(runs - 1 - k);
                assert!(visits - spent[0] >= VISITS_SHARE, "{runs} {k}");
                assert!(asks - spent[1] >= ASKS_SHARE, "{runs} {k}");
                spent = [visits + OVERRUN, asks];
            }
        }
        assert_eq!(caps(0), [VISITS_A_FRAME, ASKS_A_FRAME]);
    }
}
