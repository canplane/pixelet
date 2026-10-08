// pxl-kernel/layer/terrain/fixed.rs

//! `.fixed.terrain`: the room every resident terrain tile lies in, and where
//! a record's bytes are. A plane is one lane of every frame end to end;
//! planes in order: ground, the separation's eight, elevation, index, coarser
//! index cells. The elevation is its own plane because its 2,592-byte record
//! is 32 past a cache line; held with the ground it would misalign every
//! second tile's lanes.

use pxl_spec::bake::constant::ELE_PADDED_SPAN;
use pxl_spec::bake::format::GroundId;
use pxl_spec::bake::format::COARSE_LANE;
use pxl_spec::bake::separation::{MAX_BOXES, MAX_LINES, MAX_POINTS};
use pxl_spec::space::layer::lattice::{EXTENT, TILE_CELLS};

use crate::layer::residency::Residency;
use pxl_spec::bake::format::TerrainTile;

/// `.fixed.terrain`, handed in at boot.
pub static RANGE: crate::mem::fixed::Held = crate::mem::fixed::Held::new();

const FRAMES: u32 = Residency::<TerrainTile>::FRAMES;

/// One tile's ground: a `GroundId` a cell, no stride and no padding.
pub const GROUND_LANE_BYTES: u32 = TILE_CELLS * TILE_CELLS * core::mem::size_of::<GroundId>() as u32;

pub const GROUND_BYTES: u32 = FRAMES * GROUND_LANE_BYTES;

/// Rounded up to a whole cache line.
const fn separation_lane(count: usize, size: usize) -> u32 {
    ((count * size).div_ceil(64) * 64) as u32
}

/// The separation's lanes, a plane each in order: line starts, points, tops,
/// width, kind, form, clear, boxes.
pub const SEPARATION_LANE_BYTES: [u32; 8] = [
    separation_lane(MAX_LINES + 1, 4),
    separation_lane(MAX_POINTS, 4),
    separation_lane(MAX_POINTS, 2),
    separation_lane(MAX_LINES, 1),
    separation_lane(MAX_LINES, 1),
    separation_lane(MAX_LINES, 1),
    separation_lane(MAX_LINES, 1),
    separation_lane(MAX_BOXES, 8),
];

pub const SEPARATION_BYTES: u32 = FRAMES * {
    let mut sum = 0;
    let mut k = 0;
    while k < SEPARATION_LANE_BYTES.len() {
        sum += SEPARATION_LANE_BYTES[k];
        k += 1;
    }
    sum
};

/// Inner samples plus the two-cell skirt bicubic reads at the edges.
pub const ELE_LANE_BYTES: u32 = ELE_PADDED_SPAN * ELE_PADDED_SPAN * 2;

pub const ELE_PLANE_BYTES: u32 = FRAMES * ELE_LANE_BYTES;

/// A cell's extent, as the index lane and its coarser cells both hold it.
const EXTENT_BYTES: u32 = core::mem::size_of::<[i16; 2]>() as u32;

pub const EXTENT_LANE_BYTES: u32 = EXTENT.tile_cells() * EXTENT.tile_cells() * EXTENT_BYTES;

pub const EXTENT_PLANE_BYTES: u32 = FRAMES * EXTENT_LANE_BYTES;

pub const COARSE_LANE_BYTES: u32 = COARSE_LANE as u32 * EXTENT_BYTES;

pub const COARSE_PLANE_BYTES: u32 = FRAMES * COARSE_LANE_BYTES;

pub const TERRAIN_BYTES: u32 = GROUND_BYTES + SEPARATION_BYTES + ELE_PLANE_BYTES + EXTENT_PLANE_BYTES + COARSE_PLANE_BYTES;

const _: () = assert!(
    GROUND_LANE_BYTES.is_multiple_of(64),
    "terrain: a ground lane must be whole cache lines, or every second record's lane starts part way into one"
);

pub const fn ground_off(record: u32) -> u32 {
    record * GROUND_LANE_BYTES
}

pub const fn separation_off(record: u32, lane: usize) -> u32 {
    let mut plane = GROUND_BYTES;
    let mut k = 0;
    while k < lane {
        plane += FRAMES * SEPARATION_LANE_BYTES[k];
        k += 1;
    }
    plane + record * SEPARATION_LANE_BYTES[lane]
}

pub const fn ele_off(record: u32) -> u32 {
    GROUND_BYTES + SEPARATION_BYTES + record * ELE_LANE_BYTES
}

pub const fn extent_off(record: u32) -> u32 {
    GROUND_BYTES + SEPARATION_BYTES + ELE_PLANE_BYTES + record * EXTENT_LANE_BYTES
}

pub const fn coarse_off(record: u32) -> u32 {
    GROUND_BYTES + SEPARATION_BYTES + ELE_PLANE_BYTES + EXTENT_PLANE_BYTES + record * COARSE_LANE_BYTES
}
