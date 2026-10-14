// pxl-kernel/layer/terrain/fixed.rs

//! `.fixed.terrain`: the room every resident terrain tile lies in, and where
//! a record's bytes are. A plane is one lane of every frame end to end;
//! planes in order: ground, the separation's eight, heights, the ways'
//! bound, the bound's levels, and the words its cells come back under.

use pxl_spec::bake::constant::LEVEL_WORDS;
use pxl_spec::bake::separation::{MAX_BOXES, MAX_LINES, MAX_POINTS};
use pxl_spec::bake::tile::terrain::GroundId;
use pxl_spec::bake::tile::terrain::WAYS_LANE;
use pxl_spec::space::layer::lattice::TILE_CELLS;

use crate::layer::residency::Residency;
use pxl_spec::bake::tile::terrain::TerrainTile;

/// `.fixed.terrain`, handed in at boot.
pub static RANGE: crate::mem::fixed::Held = crate::mem::fixed::Held::new();

const FRAMES: u32 = Residency::<TerrainTile>::FRAMES;

/// One tile's ground: a cell's own look, a `GroundId` a cell, no stride and no padding.
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

/// One tile's heights: an i16 a cell, as the GPU drew them, read back.
pub const HEIGHT_LANE_BYTES: u32 = TILE_CELLS * TILE_CELLS * 2;

pub const HEIGHT_PLANE_BYTES: u32 = FRAMES * HEIGHT_LANE_BYTES;

/// One tile's ways' bound by square and its levels, made from its lines
/// where it lands (pxl-spec bake/tile/terrain.rs `ways_of`).
pub const WAYS_LANE_BYTES: u32 = WAYS_LANE as u32 * 4;

pub const WAYS_PLANE_BYTES: u32 = FRAMES * WAYS_LANE_BYTES;

/// One tile's bound's levels over its cells, as the GPU made them
/// (pxl-portable `ground::join`), read back: a word an entry.
pub const LEVEL_LANE_BYTES: u32 = LEVEL_WORDS * 4;

pub const LEVEL_PLANE_BYTES: u32 = FRAMES * LEVEL_LANE_BYTES;

/// One tile's two words its cells come back under, its heights' and its
/// levels' (pxl-kernel layer/terrain/tile.rs `back`).
pub const BACK_LANE_BYTES: u32 = 8;

pub const BACK_PLANE_BYTES: u32 = FRAMES * BACK_LANE_BYTES;

pub const TERRAIN_BYTES: u32 = GROUND_BYTES + SEPARATION_BYTES + HEIGHT_PLANE_BYTES + WAYS_PLANE_BYTES + LEVEL_PLANE_BYTES + BACK_PLANE_BYTES;

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

pub const fn height_off(record: u32) -> u32 {
    GROUND_BYTES + SEPARATION_BYTES + record * HEIGHT_LANE_BYTES
}

pub const fn ways_off(record: u32) -> u32 {
    GROUND_BYTES + SEPARATION_BYTES + HEIGHT_PLANE_BYTES + record * WAYS_LANE_BYTES
}

pub const fn level_off(record: u32) -> u32 {
    GROUND_BYTES + SEPARATION_BYTES + HEIGHT_PLANE_BYTES + WAYS_PLANE_BYTES + record * LEVEL_LANE_BYTES
}

pub const fn back_off(record: u32) -> u32 {
    GROUND_BYTES + SEPARATION_BYTES + HEIGHT_PLANE_BYTES + WAYS_PLANE_BYTES + LEVEL_PLANE_BYTES + record * BACK_LANE_BYTES
}
