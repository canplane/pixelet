// pxl-kernel/layer/terrain/fixed.rs

//! `.fixed.terrain`: the room every resident terrain page lies in, and where
//! a record's bytes are. Planes in order: ground, seven deck lanes,
//! elevation, index lane, index blocks. The elevation is its own plane
//! because its 2,592-byte record is 32 past a cache line; held with the
//! ground it would misalign every second page's lanes.

use pxl_spec::bake::constant::ELE_PADDED_SPAN;
use pxl_spec::bake::deck::{MAX_BOXES, MAX_DECKS, MAX_POINTS};
use pxl_spec::bake::format::{Block, BLOCK_LANE};
use pxl_spec::bake::pack::GroundCell;
use pxl_spec::space::pyramid::{EXTENT, PAGE_CELLS};

use crate::world::paging::RECORDS;

/// One page's ground: a `GroundCell` a cell, no stride and no padding.
pub const GROUND_LANE_BYTES: u32 = PAGE_CELLS * PAGE_CELLS * core::mem::size_of::<GroundCell>() as u32;

pub const GROUND_BYTES: u32 = RECORDS * GROUND_LANE_BYTES;

/// Rounded up to a whole cache line.
const fn deck_lane(count: usize, size: usize) -> u32 {
    ((count * size).div_ceil(64) * 64) as u32
}

/// Plane order: deck starts, points, tops, width, kind, form, boxes.
pub const DECK_LANE_BYTES: [u32; 7] = [
    deck_lane(MAX_DECKS + 1, 4),
    deck_lane(MAX_POINTS, 4),
    deck_lane(MAX_POINTS, 2),
    deck_lane(MAX_DECKS, 1),
    deck_lane(MAX_DECKS, 1),
    deck_lane(MAX_DECKS, 1),
    deck_lane(MAX_BOXES, 8),
];

pub const DECK_BYTES: u32 = RECORDS * {
    let mut sum = 0;
    let mut k = 0;
    while k < DECK_LANE_BYTES.len() {
        sum += DECK_LANE_BYTES[k];
        k += 1;
    }
    sum
};

/// Inner samples plus the two-cell skirt bicubic reads at the edges.
pub const ELE_LANE_BYTES: u32 = ELE_PADDED_SPAN * ELE_PADDED_SPAN * 2;

pub const ELE_PLANE_BYTES: u32 = RECORDS * ELE_LANE_BYTES;

/// Two i16 a cell of the index lattice.
pub const EXTENT_LANE_BYTES: u32 = EXTENT.page_cells() * EXTENT.page_cells() * 4;

pub const EXTENT_PLANE_BYTES: u32 = RECORDS * EXTENT_LANE_BYTES;

pub const BLOCK_LANE_BYTES: u32 = BLOCK_LANE as u32 * core::mem::size_of::<Block>() as u32;

pub const BLOCK_PLANE_BYTES: u32 = RECORDS * BLOCK_LANE_BYTES;

pub const TERRAIN_BYTES: u32 = GROUND_BYTES + DECK_BYTES + ELE_PLANE_BYTES + EXTENT_PLANE_BYTES + BLOCK_PLANE_BYTES;

const _: () = assert!(
    GROUND_LANE_BYTES.is_multiple_of(64),
    "terrain: a ground lane must be whole cache lines, or every second record's lane starts part way into one"
);

pub const fn ground_off(record: u32) -> u32 {
    record * GROUND_LANE_BYTES
}

pub const fn deck_off(record: u32, lane: usize) -> u32 {
    let mut plane = GROUND_BYTES;
    let mut k = 0;
    while k < lane {
        plane += RECORDS * DECK_LANE_BYTES[k];
        k += 1;
    }
    plane + record * DECK_LANE_BYTES[lane]
}

pub const fn ele_off(record: u32) -> u32 {
    GROUND_BYTES + DECK_BYTES + record * ELE_LANE_BYTES
}

pub const fn extent_off(record: u32) -> u32 {
    GROUND_BYTES + DECK_BYTES + ELE_PLANE_BYTES + record * EXTENT_LANE_BYTES
}

pub const fn blocks_off(record: u32) -> u32 {
    GROUND_BYTES + DECK_BYTES + ELE_PLANE_BYTES + EXTENT_PLANE_BYTES + record * BLOCK_LANE_BYTES
}
