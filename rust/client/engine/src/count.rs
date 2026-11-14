// pxl-engine/count.rs

//! wgsl-port: the counters of what a cast did, kept only while the `ray_cnt` setting
//! is on. The eye and the light are cast by the same passes, and each of them has its own
//! run of `LANE_CNT` counters in one array (`at`), which pxl-render ray/count.rs reads back
//! and sums.
//!
//! No pass has a binding left for the array itself, so each adds its counts to a buffer it
//! already writes. A walk that writes the node cache adds to the node cache's head
//! (`node_cache::lane_at`). A pixel, or a light slot, adds to the arguments of the pass that
//! walks the missed pixels again (`ray::MISS_ARGS_CNTS`), where it already counts itself
//! when it lists a hole. The boxes drawn are the raster's instance count
//! (`raster::ARGS_INSTANCE_CNT`). The frame's last copies gather these into the array.

/// The observers, each with a run of `LANE_CNT` counters in the array.
pub const EYE: u32 = 0;
pub const LIGHT: u32 = 1;
pub const OBSERVER_CNT: u32 = 2;

/// An observer's counters. `BEAM`, `SAMPLE` and `HOLE_WALK` count the nodes visited by its
/// beam rays, its samples and the walk of its holes. `ASK` counts the nodes its walks asked of
/// the world, which no node cache slot answered. `VOXEL`, `HOLE` and `SKY` count the pixels or
/// slots it cast again by what each took: the visible voxel, a walk of its own as a hole, or
/// the sky. `BOX` counts the boxes it drew.
pub const BEAM: u32 = 0;
pub const SAMPLE: u32 = 1;
pub const HOLE_WALK: u32 = 2;
pub const ASK: u32 = 3;
pub const VOXEL: u32 = 4;
pub const HOLE: u32 = 5;
pub const SKY: u32 = 6;
pub const BOX: u32 = 7;
pub const LANE_CNT: u32 = 8;
pub const WORDS: u32 = OBSERVER_CNT * LANE_CNT;

/// A walk that writes the node cache adds to the first `WALK_CNT` counters, `BEAM` to `ASK`,
/// and a pixel or a slot adds to the `RAY_CNT` counters after them, `VOXEL` to `SKY`. Each of
/// the two runs lies in this order in the buffer it is added to.
pub const WALK_CNT: u32 = 4;
pub const RAY_CNT: u32 = 3;
const _: () = assert!(
    BEAM == 0
        && ASK + 1 == WALK_CNT
        && VOXEL == WALK_CNT
        && SKY + 1 == WALK_CNT + RAY_CNT
        && BOX == SKY + 1
);

pub const fn at(observer: u32, lane: u32) -> u32 {
    observer * LANE_CNT + lane
}
