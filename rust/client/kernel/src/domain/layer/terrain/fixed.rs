// pxl-kernel/domain/layer/terrain/fixed.rs

//! `.fixed.terrain`: the room every resident terrain tile and every held
//! square lies in, and where a record's bytes are. A plane is one lane of
//! every frame, or of every square, end to end; planes in order: the
//! separation's seven, a tile's; then a square's cells; then
//! the eye's windows' levels over their squares as the GPU folded them,
//! once; then the room a fill works in, once. The squares' ways' trees are
//! the heap's, grown as they are taken (mirror.rs), and so are a landing
//! square's records, the renderer's (pxl-render gpu/ray/ground.rs
//! `GroundBuffers::room`).

use pxl_spec::bake::separation::{MAX_LINES, MAX_POINTS};
use pxl_spec::bake::tile::terrain::LOOK_LANE;
use pxl_spec_core::window::{BACK_WORDS, LINES_WORDS, SQUARE_CELLS, WAYS_SQUARE_SEGMENTS};

use crate::domain::layer::residency::Residency;
use crate::sys::mem::fixed::lane_bytes;
use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec_core::window::SQUARES;

/// `.fixed.terrain`, handed in at boot.
pub static RANGE: crate::sys::mem::fixed::Held = crate::sys::mem::fixed::Held::new();

const FRAMES: u32 = Residency::<TerrainTile>::FRAMES;

/// The separation's lanes, a plane each in order: line starts, points, tops,
/// width, kind, form, clear; each its length and its element's bytes, which
/// pxl-spec `separation::Room` is laid on (domain/layer/terrain/tile.rs `room_of`).
pub const SEPARATION_LANES: [(usize, usize); 7] = [
    (MAX_LINES + 1, 4),
    (MAX_POINTS, 4),
    (MAX_POINTS, 2),
    (MAX_LINES, 1),
    (MAX_LINES, 1),
    (MAX_LINES, 1),
    (MAX_LINES, 1),
];

/// Separation lane `k`'s bytes in a record.
const fn separation_lane_bytes(k: usize) -> u32 {
    let (count, size) = SEPARATION_LANES[k];
    lane_bytes(count, size)
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
/// its ways' tree's root (pxl-spec-core `window::BACK_WORDS`).
pub const CELLS_BYTES: u32 = BACK_WORDS * 4;
pub const CELLS_LANE_BYTES: u32 = lane_bytes(CELLS_BYTES as usize, 1);

/// The room a fill works in (window.rs `fill`), a lane each in order: a
/// tile's looks four a cell as handed, a square's cut from them, its lines,
/// and the pairs `square_lines` lays them with.
pub const FILL_LANE_BYTES: [u32; 4] = [
    lane_bytes(LOOK_LANE, 1),
    lane_bytes(4 * SQUARE_CELLS as usize, 1),
    lane_bytes(LINES_WORDS as usize, 4),
    lane_bytes(WAYS_SQUARE_SEGMENTS as usize, 8),
];

/// Where the fill's room starts, on a cache line.
const FILL_AT: u32 = lane_bytes(cells_off(SQUARES) as usize, 1);

pub const TERRAIN_BYTES: u32 = FILL_AT + sum(&FILL_LANE_BYTES);

/// Record `record`'s separation lane `k`: where it starts, and its bytes.
pub const fn separation_lane(record: u32, k: usize) -> (u32, u32) {
    let bytes = separation_lane_bytes(k);
    (separation_plane(k) + record * bytes, bytes)
}

/// Where berth `berth`'s cells come back to (domain/layer/terrain/window.rs
/// `Berth`), `SQUARES` of them.
pub const fn cells_off(berth: u32) -> u32 {
    SEPARATION_BYTES + berth * CELLS_LANE_BYTES
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
