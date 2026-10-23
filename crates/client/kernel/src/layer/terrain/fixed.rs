// pxl-kernel/layer/terrain/fixed.rs

//! `.fixed.terrain`: the room every resident terrain tile and every held
//! square lies in, and where a record's bytes are. A plane is one lane of
//! every frame, or of every square, end to end; planes in order: the
//! separation's eight, a tile's; then a square's cells, its looks, and the
//! word its cells come back under; then the room a fill works in, once.

use pxl_portable::window::{BACK_WORDS, LINES_WORDS, SQUARE_CELLS, WAYS_SQUARE_SEGMENTS};
use pxl_spec::bake::separation::{MAX_BOXES, MAX_LINES, MAX_POINTS};
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
/// width, kind, form, clear, boxes.
pub const SEPARATION_LANE_BYTES: [u32; 8] = [
    lane(MAX_LINES + 1, 4),
    lane(MAX_POINTS, 4),
    lane(MAX_POINTS, 2),
    lane(MAX_LINES, 1),
    lane(MAX_LINES, 1),
    lane(MAX_LINES, 1),
    lane(MAX_LINES, 1),
    lane(MAX_BOXES, 8),
];

const fn sum(lanes: &[u32]) -> u32 {
    let mut sum = 0;
    let mut k = 0;
    while k < lanes.len() {
        sum += lanes[k];
        k += 1;
    }
    sum
}

pub const SEPARATION_BYTES: u32 = FRAMES * sum(&SEPARATION_LANE_BYTES);

/// One square's cells as the GPU drew them, read back whole: each cell's
/// height, an i16, then its levels and its ways' bound, a word an entry
/// (pxl-portable `window::BACK_WORDS`).
pub const CELLS_BYTES: u32 = BACK_WORDS * 4;
pub const CELLS_LANE_BYTES: u32 = lane(CELLS_BYTES as usize, 1);

/// One square's looks: a cell's own, a byte.
pub const LOOK_LANE_BYTES: u32 = SQUARE_CELLS;

/// The word a square's cells come back under (pxl-render `Renderer::read_back`).
pub const BACK_LANE_BYTES: u32 = 4;

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
const FILL_AT: u32 = lane((SEPARATION_BYTES + SQUARES * (CELLS_LANE_BYTES + LOOK_LANE_BYTES + BACK_LANE_BYTES)) as usize, 1);

pub const TERRAIN_BYTES: u32 = FILL_AT + sum(&FILL_LANE_BYTES);

const _: () = assert!(LOOK_LANE_BYTES.is_multiple_of(64), "terrain: a look lane must be whole cache lines");

pub const fn separation_off(record: u32, lane: usize) -> u32 {
    let mut plane = 0;
    let mut k = 0;
    while k < lane {
        plane += FRAMES * SEPARATION_LANE_BYTES[k];
        k += 1;
    }
    plane + record * SEPARATION_LANE_BYTES[lane]
}

pub const fn cells_off(square: u32) -> u32 {
    SEPARATION_BYTES + square * CELLS_LANE_BYTES
}

pub const fn looks_off(square: u32) -> u32 {
    SEPARATION_BYTES + SQUARES * CELLS_LANE_BYTES + square * LOOK_LANE_BYTES
}

pub const fn back_off(square: u32) -> u32 {
    SEPARATION_BYTES + SQUARES * (CELLS_LANE_BYTES + LOOK_LANE_BYTES) + square * BACK_LANE_BYTES
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
