// pxl-engine/count.rs

//! gpu-portable: what a cast walked and reused, counted where the
//! `ray_counts` setting asks, one array an observer's lanes alike (`at`):
//! the eye's and the light's, each cast by the same passes — its beam rays,
//! its samples, its walk again, its pixels or slots taking the visible
//! sample or walking, and the boxes its samples drew (pxl-render
//! gpu/ray/count.rs). A pass adds where it can write: a walk that writes
//! the node cache into its head (`node_cache::lane_at`), as no binding
//! more fits beside it; a pixel or a slot into its walk again's args
//! (`ray::MISS_ARGS_COUNT`), which it adds to already, listing itself; the
//! boxes off the raster's draw (`raster::ARGS_INSTANCES`). The frame's last
//! copies gather them into the array, which is read back and summed.

/// The observers, an array's run of `LANES` each.
pub const EYE: u32 = 0;
pub const LIGHT: u32 = 1;
pub const OBSERVERS: u32 = 2;

/// The kinds of run a frame's budget of work is shared among (`ray::caps`), in the order a frame
/// runs them: the eye's lattice, its holes, the light's lattice, its holes. Each holds its caps
/// in the node cache's head (`node_cache::caps_at`).
pub const EYE_LATTICE: u32 = 0;
pub const EYE_HOLES: u32 = 1;
pub const LIGHT_LATTICE: u32 = 2;
pub const LIGHT_HOLES: u32 = 3;
pub const RUNS: u32 = 4;

/// An observer's lanes: the nodes its beam rays, its samples and its walk
/// again asked (`BEAM`, `SAMPLE`, `MISS`), its walks' visits; of its walks
/// that ask the world by hand, its samples' and the light's walk again,
/// those asked of the world (`ASKED`), no slot answering; its pixels or
/// slots that reused the visible voxel, those that walked and those that took
/// the sky (`REUSED`, `WALKED`, `SKY`), together every one it cast again; the
/// boxes drawn (`BOXES`).
pub const BEAM: u32 = 0;
pub const SAMPLE: u32 = 1;
pub const MISS: u32 = 2;
pub const ASKED: u32 = 3;
pub const REUSED: u32 = 4;
pub const WALKED: u32 = 5;
pub const SKY: u32 = 6;
pub const BOXES: u32 = 7;
pub const LANES: u32 = 8;
/// Past the lanes, each observer's owed work as its last cast or queue left it
/// (`node_cache::OWED_AT`), a word an observer: kept in a word of its own each frame the
/// observer's queue runs, counted or not, copied in by the first copy free and read back with
/// the lanes, so the renderer asks for the next frame while it is not none (pxl-render
/// gpu/ray/count.rs `Readback`, `Counter`).
pub const OWED: u32 = OBSERVERS * LANES;
/// The bit of the eye's owed word that says its rolled lattice owes an offset
/// (`beam::ROLLS`, `beam::ROLL_DONE`), set by the close of every frame its queue runs
/// (pxl-render roll.wgsl): the renderer casts while it reads it (gpu/ray/count.rs
/// `Readback::lattice`). The word's other bits count its parked pixels.
pub const LATTICE_OWED: u32 = 1 << 31;
pub const WORDS: u32 = OWED + OBSERVERS;

/// The lanes a walk writing the node cache adds to, from the first
/// (`BEAM` to `ASKED`), and those a pixel or a slot does (`REUSED` to
/// `SKY`), each a run where it adds.
pub const WALKS: u32 = 4;
pub const RAYS: u32 = 3;
const _: () = assert!(
    BEAM == 0
        && ASKED + 1 == WALKS
        && REUSED == WALKS
        && SKY + 1 == WALKS + RAYS
        && BOXES == SKY + 1
);

/// Lane `lane` of `observer` in the array.
pub const fn at(observer: u32, lane: u32) -> u32 {
    observer * LANES + lane
}
