// pxl-spec-core/tally.rs

//! gpu-portable: what a cast walked and took, counted where the
//! `ray_counts` setting asks, one array an observer's lanes alike (`at`):
//! the eye's and the sun's, each cast by the same passes — its beam rays,
//! its samples, its rays a pixel or a slot taking the raster's pick or
//! walking, those walked again, and the boxes its samples drew
//! (pxl-render gpu/ray/tally.rs). A pass adds where it can write: a walk
//! that writes the observed pyramid into its head (`observed::TALLY_AT`, `walk_at`),
//! as no binding more fits beside it; a pixel's or a slot's into its walk
//! again's args (`ray::MISS_ARGS_TALLY`), which it adds to already, as its
//! read of the pyramid is no atomic one; the boxes off the raster's draw
//! (`raster::ARGS_INSTANCES`). The frame's last copies gather them into the
//! array, which is read back and summed.

/// The observers, an array's run of `LANES` each.
pub const EYE: u32 = 0;
pub const SUN: u32 = 1;
pub const OBSERVERS: u32 = 2;

/// An observer's lanes: the nodes its beam rays, its samples and its walk
/// again asked (`BEAM`, `SAMPLE`, `MISS`), its walks' visits; its pixels or
/// slots that took the raster's pick and those that walked (`PICKED`,
/// `WALKED`), and the nodes those walked asked (`PIXEL`); the boxes drawn
/// (`BOXES`).
pub const BEAM: u32 = 0;
pub const SAMPLE: u32 = 1;
pub const MISS: u32 = 2;
pub const PICKED: u32 = 3;
pub const WALKED: u32 = 4;
pub const PIXEL: u32 = 5;
pub const BOXES: u32 = 6;
pub const LANES: u32 = 7;
pub const WORDS: u32 = OBSERVERS * LANES;

/// The lanes a walk writing the pyramid adds to, from the first
/// (`BEAM` to `MISS`), and those a pixel or a slot does (`PICKED` to
/// `PIXEL`), each a run where it adds.
pub const WALKS: u32 = 3;
pub const RAYS: u32 = 3;
const _: () =
    assert!(BEAM == 0 && MISS + 1 == WALKS && PICKED == WALKS && PIXEL + 1 == WALKS + RAYS);

const _: () = assert!(
    OBSERVERS * WALKS == crate::observed::TALLY_WORDS,
    "the walks' counts fill the pyramid's head words for them"
);

/// Where `observer`'s lane `lane` of its walks is counted in the observed
/// pyramid's head (`observed::TALLY_AT`).
pub const fn walk_at(observer: u32, lane: u32) -> u32 {
    crate::observed::TALLY_AT + observer * WALKS + lane
}

/// Lane `lane` of `observer` in the array.
pub const fn at(observer: u32, lane: u32) -> u32 {
    observer * LANES + lane
}
