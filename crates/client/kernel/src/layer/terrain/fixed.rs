// pxl-kernel/layer/terrain/fixed.rs

//! `.fixed.terrain`: the room every resident terrain tile and every held
//! square lies in, and where a record's bytes are. A plane is one lane of
//! every frame, or of every square, end to end; planes in order: the
//! separation's seven, a tile's; then a square's cells; then
//! the eye's windows' levels over their squares as the GPU folded them,
//! once; then a landing square's ways' records, a read-back each; then the
//! CPU's mirror of the GPU's ways' pool, once; then the room a fill works
//! in, once.

use pxl_portable::ways::tree::{POOL_BLOCKS, RECORDS_BYTES};
use pxl_portable::window::{BACK_WORDS, EYE_JOINED_WORDS, LINES_WORDS, READS, SQUARE_CELLS, WAYS_SQUARE_SEGMENTS};
use pxl_spec::bake::separation::{MAX_LINES, MAX_POINTS};
use pxl_spec::bake::tile::terrain::LOOK_LANE;

use crate::layer::residency::Residency;
use pxl_portable::window::SQUARES;
use pxl_spec::bake::tile::terrain::TerrainTile;

/// `.fixed.terrain`, handed in at boot.
pub static RANGE: crate::mem::fixed::Held = crate::mem::fixed::Held::new();

const FRAMES: u32 = Residency::<TerrainTile>::FRAMES;

/// Rounded up to a whole cache line.
const fn lane(count: usize, size: usize) -> u32 {
    ((count * size).div_ceil(64) * 64) as u32
}

/// The separation's lanes, a plane each in order: line starts, points, tops,
/// width, kind, form, clear; each its length and its element's bytes, which
/// pxl-spec `separation::Room` is laid on (layer/terrain/tile.rs `room_of`).
pub const SEPARATION_LANES: [(usize, usize); 7] = [(MAX_LINES + 1, 4), (MAX_POINTS, 4), (MAX_POINTS, 2), (MAX_LINES, 1), (MAX_LINES, 1), (MAX_LINES, 1), (MAX_LINES, 1)];

/// Separation lane `k`'s bytes in a record.
const fn separation_lane_bytes(k: usize) -> u32 {
    let (count, size) = SEPARATION_LANES[k];
    lane(count, size)
}

/// Where separation lane `k`'s plane starts; past the last, where the planes end.
const fn separation_plane(k: usize) -> u32 {
    let mut at = 0;
    let mut j = 0;
    while j < k {
        at += FRAMES * separation_lane_bytes(j);
        j += 1;
    }
    at
}

const fn sum(lanes: &[u32]) -> u32 {
    let mut sum = 0;
    let mut k = 0;
    while k < lanes.len() {
        sum += lanes[k];
        k += 1;
    }
    sum
}

pub const SEPARATION_BYTES: u32 = separation_plane(SEPARATION_LANES.len());

/// One square's cells as the GPU drew them, read back whole: its columns,
/// a word a cell (its ground's top), then its levels, its cells as laid and
/// its ways' tree's root (pxl-portable `window::BACK_WORDS`).
pub const CELLS_BYTES: u32 = BACK_WORDS * 4;
pub const CELLS_LANE_BYTES: u32 = lane(CELLS_BYTES as usize, 1);

/// The eye's windows' levels over their squares (pxl-portable
/// `window::fold`), read back as the GPU folded them (pxl-render
/// `GroundSquares::joined`).
pub const JOINED_BYTES: u32 = lane(EYE_JOINED_WORDS as usize, 4);

/// A landing square's ways' records (pxl-portable `ways::tree::RECORDS_WORDS`),
/// a room a read-back, read once it lands into the pool's mirror.
pub const RECORDS_ROOM_BYTES: u32 = READS * RECORDS_BYTES;

/// The GPU's ways' pool as the CPU mirrors it, block for block at the same
/// slots (pxl-portable `ways::tree::Read`): its heap words, then its data.
pub const POOL_BYTES: u32 = lane(16 * POOL_BLOCKS as usize, 4);

/// The room a fill works in (window.rs `fill`), a lane each in order: a
/// tile's looks four a cell as handed, a square's cut from them, its lines,
/// and the pairs `square_lines` lays them with.
pub const FILL_LANE_BYTES: [u32; 4] = [
    lane(LOOK_LANE, 1),
    lane(4 * SQUARE_CELLS as usize, 1),
    lane(LINES_WORDS as usize, 4),
    lane(WAYS_SQUARE_SEGMENTS as usize, 8),
];

/// Where the fill's room starts, on a cache line.
const FILL_AT: u32 = pool_off() + POOL_BYTES;

pub const TERRAIN_BYTES: u32 = FILL_AT + sum(&FILL_LANE_BYTES);

/// Record `record`'s separation lane `k`: where it starts, and its bytes.
pub const fn separation_lane(record: u32, k: usize) -> (u32, u32) {
    let bytes = separation_lane_bytes(k);
    (separation_plane(k) + record * bytes, bytes)
}

pub const fn cells_off(square: u32) -> u32 {
    SEPARATION_BYTES + square * CELLS_LANE_BYTES
}

pub const fn joined_off() -> u32 {
    cells_off(SQUARES)
}

pub const fn records_off() -> u32 {
    lane((joined_off() + JOINED_BYTES) as usize, 1)
}

pub const fn pool_off() -> u32 {
    records_off() + RECORDS_ROOM_BYTES
}

pub const fn fill_off(lane: usize) -> u32 {
    let mut at = FILL_AT;
    let mut k = 0;
    while k < lane {
        at += FILL_LANE_BYTES[k];
        k += 1;
    }
    at
}
