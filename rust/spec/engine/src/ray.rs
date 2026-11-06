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

/// The eye's walked pixels, past the camera grid's words in the beam rays' buffer
/// (`beam::Grid::walked_at`), room for every pixel drawn, so no list of them is ever full: the
/// pixels the reading walk marked (`MISS`), its row over its column, a word each
/// (`WALKED_LIST`), after their count, copied in from the walk's dispatch (`MISS_ARGS_CNT`)
/// once the reading walk is done (`WALKED_CNT`). A pixel is listed once a cast, so the list
/// holds at most the pixels drawn. `drawn` is the rays' pixels, across times down; each place
/// is a word from `Grid::walked_at`.
pub const WALKED_CNT: u32 = 0;
pub const WALKED_LIST: u32 = 1;

pub const fn walked_words(drawn: u32) -> u32 {
    WALKED_LIST + drawn
}

/// What a walk over the node cache left, listed for the walk that asks the world (pxl-render
/// list.wgsl `enlist`; beam_hit.wgsl, sample_hit.wgsl):
/// its dispatch, laid as `MISS_ARGS_LIST`'s, no row until one is listed, then its count
/// (`LEFT_CNT`). Every list, these and the walk again's, raises the same row word.
pub const LEFT_EMPTY: [u32; 4] = [MISS_ACROSS, 0, 1, 0];
pub const LEFT_CNT: u32 = 3;
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
/// eye's pixel while the rays stand, `node_cache::RESUMES`); the asks, a lane a node a walk was
/// parked at, once a node; the second walk, over the node cache alone, over the rays the first
/// parked, from the state each was parked in; the asks again; and the last walk, which asks
/// the world where no slot keeps a node, over the rays the second parked. Both observers' walks
/// count their work against the frame's budget (`VISITS_A_FRAME`, `ASKS_A_FRAME`) up to their
/// caps (`caps`): a hole the budget does not reach stays parked and is owed
/// (`node_cache::OWED_AT`), a light's slot's tile to the light's next claim, an eye's pixel to
/// the next frame's queue while the rays stand.
pub const QUEUE_LANES: u32 = 64;
pub const QUEUE_GROUPS: u32 = 256;
pub const QUEUE_STEPS: u32 = 16;

/// The work the holes' queues may do in a frame, the eye's and the light's together
/// (pxl-render queue_walk.wgsl, queue_ask.wgsl, queue_last.wgsl): the nodes their walks visit,
/// and the nodes they ask of the world. A hole the budget does not reach is parked and owed
/// (`node_cache::OWED_AT`). To be set from the S23's hot drive.
pub const VISITS_A_FRAME: u32 = 1 << 23;
pub const ASKS_A_FRAME: u32 = 1 << 16;

/// The share of a budget reserved for each observer whose queue runs in a frame, so neither
/// starves the other: the queue that runs first stops short of the budget by this and by
/// `OVERRUN` where the other runs after it (`caps`).
pub const fn reserved(budget: u32) -> u32 {
    budget / 4
}

/// The most a walk's lanes visit past a cap: every lane of the dispatch saw the visits under
/// it and walked one run of steps more. Asks are counted one at a time and pass no cap.
pub const OVERRUN: u32 = QUEUE_GROUPS * QUEUE_LANES * QUEUE_STEPS;
const _: () = assert!(
    VISITS_A_FRAME - reserved(VISITS_A_FRAME) - OVERRUN >= reserved(VISITS_A_FRAME),
    "the first queue's cap keeps a reserved share of its own"
);

/// The caps a queue's work counts to, the frame's work so far included, the visits' and the
/// asks' (`QUEUE_CAPS`): the whole budget where it runs last or alone; short of it by the
/// share reserved for the queue after it, and by what its lanes may overrun, where it runs
/// `first` and another runs after it (`shared`). So the queue after it does at least the
/// reserved share of each.
pub const fn caps(first: bool, shared: bool) -> [u32; 2] {
    if first && shared {
        [
            VISITS_A_FRAME - reserved(VISITS_A_FRAME) - OVERRUN,
            ASKS_A_FRAME - reserved(ASKS_A_FRAME),
        ]
    } else {
        [VISITS_A_FRAME, ASKS_A_FRAME]
    }
}

/// The queue's words, an observer's buffer of them (pxl-render queue_walk.wgsl
/// `queue`, queue_ask.wgsl): the next hole the first walk takes (`QUEUE_TAKEN`) and the next the
/// second takes (`QUEUE_TAKEN_AGAIN`); the nodes each walk was parked at, listed for the asks
/// (`QUEUE_ASKS`, `QUEUE_ASKS_AGAIN`); the work its walks over the node cache did, the nodes
/// visited and those asked, the frame's work before it included, which the budget counts and
/// its last walk goes on from (`QUEUE_VISITED`, `QUEUE_ASKED`; `node_cache::VISITED`); the caps
/// they count to, the host's each frame (`QUEUE_CAPS`, `caps`); the stops the two walks
/// listed, as `node_cache::HITS_ARGS` lays them (`QUEUE_HITS`); then the asks' records
/// (`QUEUE_RECORDS_AT`, `QUEUE_RECORD_WORDS` each, `QUEUE_RECORDS_ROOM` a walk: as many as the
/// frame may ask). Each frame's start is `QUEUE_EMPTY`.
pub const QUEUE_TAKEN: u32 = 0;
pub const QUEUE_TAKEN_AGAIN: u32 = 1;
pub const QUEUE_ASKS: u32 = 2;
pub const QUEUE_ASKS_AGAIN: u32 = 3;
pub const QUEUE_VISITED: u32 = 4;
pub const QUEUE_ASKED: u32 = 5;
pub const QUEUE_CAPS: u32 = 6;
pub const QUEUE_HITS: u32 = 8;
pub const QUEUE_RECORDS_AT: u32 = 12;
/// An ask's record: the node's zoom, its index's three words, the zooms it is asked at, far then
/// near over `ZOOM_BITS`, whether it is asked from the top, and what was handed its parent.
pub const QUEUE_RECORD_WORDS: u32 = 10;
pub const QUEUE_RECORDS_ROOM: u32 = ASKS_A_FRAME;
pub const QUEUE_WORDS: u32 = QUEUE_RECORDS_AT + 2 * QUEUE_RECORDS_ROOM * QUEUE_RECORD_WORDS;
pub const QUEUE_EMPTY: [u32; QUEUE_RECORDS_AT as usize] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0];

/// The holes' region, the last words of an observer's list, which every walk of the queue binds
/// (pxl-render queue.wgsl `state_at`), a state for each of the most holes a cast lists,
/// `room`: the eye's drawn pixels, a pixel listed once a cast (cast.wgsl, a thread a pixel), by
/// its pixel's index, its holes in its walked list (`WALKED_LIST`); and the light's slots of the
/// tiles a cast claims (light.wgsl, a thread a slot), by its tile's place and its slot within
/// the tile, a cast's and the one before's (`light::region_words`). A state is `STATE_WORDS`:
/// its distance's bits, its node's corner, and its zoom, roots and steps taken packed under
/// `PARKED`, which says the ray is parked there (pxl-render queue.wgsl `park`). Every hole can be
/// parked; none is left out.
pub const STATE_WORDS: u32 = 5;
pub const PARKED: u32 = 1 << 31;
pub const fn region_words(room: u32) -> u32 {
    room * STATE_WORDS
}
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Two queues in a frame, the first at its cap and past it by the most its lanes overrun:
    /// the second still has the reserved share of the visits and of the asks to do, and the
    /// first had at least as much; one alone has the whole budget.
    #[test]
    fn each_queue_keeps_its_reserved_share() {
        let [first_visits, first_asks] = caps(true, true);
        let [last_visits, last_asks] = caps(false, true);
        assert_eq!([last_visits, last_asks], [VISITS_A_FRAME, ASKS_A_FRAME]);
        let spent = first_visits + OVERRUN;
        assert!(last_visits - spent >= reserved(VISITS_A_FRAME));
        assert!(first_visits >= reserved(VISITS_A_FRAME));
        assert!(last_asks - first_asks >= reserved(ASKS_A_FRAME));
        assert!(first_asks >= reserved(ASKS_A_FRAME));
        assert_eq!(caps(true, false), [VISITS_A_FRAME, ASKS_A_FRAME]);
    }
}
