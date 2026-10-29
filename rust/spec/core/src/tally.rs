// pxl-spec-core/tally.rs

//! gpu-portable: what a cast walked and took, counted where the
//! `ray_counts` setting asks, one array an observer's lanes alike (`at`):
//! the eye's and the light's, each cast by the same passes — its beam rays,
//! its samples, its walk again, its pixels or slots taking the visible
//! sample or walking, and the boxes its samples drew (pxl-render
//! gpu/ray/tally.rs). A pass adds where it can write: a walk that writes
//! the observed pyramid into its head (`observed::tally_at`), as no binding
//! more fits beside it; a pixel or a slot into its walk again's args
//! (`ray::MISS_ARGS_TALLY`), which it adds to already, listing itself; the
//! boxes off the raster's draw (`raster::ARGS_INSTANCES`). The frame's last
//! copies gather them into the array, which is read back and summed.

/// The observers, an array's run of `LANES` each.
pub const EYE: u32 = 0;
pub const LIGHT: u32 = 1;
pub const OBSERVERS: u32 = 2;

/// An observer's lanes: the nodes its beam rays, its samples and its walk
/// again asked (`BEAM`, `SAMPLE`, `MISS`), its walks' visits; its pixels or
/// slots that took the visible sample and those that walked (`TOOK`,
/// `WALKED`); the boxes drawn (`BOXES`).
pub const BEAM: u32 = 0;
pub const SAMPLE: u32 = 1;
pub const MISS: u32 = 2;
pub const TOOK: u32 = 3;
pub const WALKED: u32 = 4;
pub const BOXES: u32 = 5;
pub const LANES: u32 = 6;
pub const WORDS: u32 = OBSERVERS * LANES;

/// The lanes a walk writing the pyramid adds to, from the first
/// (`BEAM` to `MISS`), and those a pixel or a slot does (`TOOK`,
/// `WALKED`), each a run where it adds.
pub const WALKS: u32 = 3;
pub const RAYS: u32 = 2;
const _: () =
    assert!(BEAM == 0 && MISS + 1 == WALKS && TOOK == WALKS && WALKED + 1 == WALKS + RAYS);

/// Lane `lane` of `observer` in the array.
pub const fn at(observer: u32, lane: u32) -> u32 {
    observer * LANES + lane
}
