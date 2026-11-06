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
///
/// Then which list the eye's holes' queue takes this frame (`holes_from_at`), the host's each
/// frame it runs: the walked list (`FROM_WALKED`), or one of the two lists of what the queue
/// before left owed (`FROM_OWED` + the list's number); and those two lists (`owed_list_at`),
/// each a count then the pixels, room for every pixel drawn. The queue's last walk lists every
/// pixel it leaves parked in the owed list it does not take (pxl-render queue_last.wgsl
/// `unreached`), its count the eye's owed word (`node_cache::OWED_AT`), copied in after it; a
/// frame that runs the eye's queue alone takes that list, so it walks only what is owed, and a
/// pixel its walk finished is never listed again.
pub const WALKED_CNT: u32 = 0;
pub const WALKED_LIST: u32 = 1;
pub const FROM_WALKED: u32 = 0;
pub const FROM_OWED: u32 = 1;

pub const fn walked_words(drawn: u32) -> u32 {
    owed_list_at(drawn, 2)
}

/// The word saying which list the eye's queue takes, of a target `drawn` pixels, from the
/// walked list's first word.
pub const fn holes_from_at(drawn: u32) -> u32 {
    WALKED_LIST + drawn
}

/// The first word, its count, of owed list `k` (0 or 1), of a target `drawn` pixels, from the
/// walked list's first word.
pub const fn owed_list_at(drawn: u32, k: u32) -> u32 {
    holes_from_at(drawn) + 1 + k * (WALKED_LIST + drawn)
}

/// The owed list the eye's queue lists what it leaves parked in, where it takes list `from`
/// (`holes_from_at`): the one it does not take.
pub const fn owed_into(from: u32) -> u32 {
    if from == FROM_OWED {
        1
    } else {
        0
    }
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
/// eye's pixel while the rays stand, `node_cache::RESUMES`); the asks, a lane a node a walk was
/// parked at, once a node; the second walk, over the node cache alone, over the rays the first
/// parked, from the state each was parked in; the asks again; and the last walk, which asks
/// the world where no slot keeps a node, over the rays the second parked. Every run counts its
/// work against the frame's budget (`VISITS_A_FRAME`, `ASKS_A_FRAME`) up to its kind's caps
/// (`caps`): a hole the budget does not reach stays parked and is owed
/// (`node_cache::OWED_AT`), a light's slot's tile to the light's next claim, an eye's pixel to
/// the next frame's queue while the rays stand.
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

/// The queue's words, an observer's buffer of them (pxl-render queue_walk.wgsl
/// `queue`, queue_ask.wgsl): the next hole the first walk takes (`QUEUE_TAKEN`) and the next the
/// second takes (`QUEUE_TAKEN_AGAIN`); the nodes each walk was parked at, listed for the asks
/// (`QUEUE_ASKS`, `QUEUE_ASKS_AGAIN`); the nodes the walk over the node cache last run visited,
/// which it counts against its budget beside the frame's (`node_cache::VISITED`) and the asks
/// after it fold into the frame's (`QUEUE_VISITED`); the stops the two walks listed, as
/// `node_cache::HITS_ARGS` lays them (`QUEUE_HITS`); then the asks' records
/// (`QUEUE_RECORDS_AT`, `QUEUE_RECORD_WORDS` each, `QUEUE_RECORDS_ROOM` a walk). Each run's
/// start is `QUEUE_EMPTY`.
pub const QUEUE_TAKEN: u32 = 0;
pub const QUEUE_TAKEN_AGAIN: u32 = 1;
pub const QUEUE_ASKS: u32 = 2;
pub const QUEUE_ASKS_AGAIN: u32 = 3;
pub const QUEUE_VISITED: u32 = 4;
pub const QUEUE_HITS: u32 = 5;
pub const QUEUE_RECORDS_AT: u32 = 9;
/// An ask's record: the node's zoom, its index's three words, the zooms it is asked at, far then
/// near over `ZOOM_BITS`, whether it is asked from the top, and what was handed its parent.
pub const QUEUE_RECORD_WORDS: u32 = 10;
/// The nodes one walk lists for the asks after it at most: a ray parked at a node listed past
/// it is asked by the last walk instead.
pub const QUEUE_RECORDS_ROOM: u32 = 1 << 16;
pub const QUEUE_WORDS: u32 = QUEUE_RECORDS_AT + 2 * QUEUE_RECORDS_ROOM * QUEUE_RECORD_WORDS;
pub const QUEUE_EMPTY: [u32; QUEUE_RECORDS_AT as usize] = [0, 0, 0, 0, 0, 0, 1, 1, 0];

/// The holes' region, the last words of an observer's list, which every walk of the queue binds
/// (pxl-render queue.wgsl `state_at`), a state for each of the most holes a cast lists,
/// `room`: the eye's drawn pixels, a pixel listed once a cast (cast.wgsl, a thread a pixel), by
/// its pixel's index, its holes in its walked list (`WALKED_LIST`); and the light's slots of the
/// tiles a cast claims (light.wgsl, a thread a slot), by its tile's place and its slot within
/// the tile, a cast's and the one before's (`light::region_words`). A state is `STATE_WORDS`:
/// its distance's bits, its node's corner, and its zoom, roots and steps taken packed under
/// `PARKED`, which says the ray is parked there (pxl-render queue.wgsl `park`). Every hole can be
/// parked; none is left out.
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
