// pxl-kernel/domain/layer/terrain/fixed.rs

//! `.fixed.terrain`: the room every resident terrain tile and every held
//! brick lies in, and where a record's bytes are. A plane is one array of
//! every frame, or of every brick, end to end; planes in order: the
//! separation's seven and the samples, a tile's; then a brick's cells and
//! its four palette indices a cell; then the room a fill works in, once. The
//! bricks' ways' trees are the heap's, grown as they are taken (pool.rs).

use pxl_engine::clipmap::{
    BRICK_CELLS, BRICK_WORDS, LINES_WORDS, SPANS_WORDS, WAYS_BRICK_SEGMENTS,
};
use pxl_engine::way::tree::{BRICK_CHILD_GROUPS, CHILD_GROUP_WORDS};
use pxl_spec::bake::separation::{MAX_LINES, MAX_POINTS};
use pxl_spec::bake::tile::terrain::{ELE_COLUMN, PALETTE_COLUMN};

use crate::domain::layer::tile_cache::TileCache;
use pxl_engine::clipmap::SLOTS;
use pxl_spec::bake::tile::terrain::TerrainTile;
use xpute_core::mem::range::arr_bytes;
use xpute_core::mem::range::OwnedRange;

/// `.fixed.terrain`, handed in at boot.
pub static RANGE: OwnedRange = OwnedRange::new();

const TILE_SLOTS: u32 = TileCache::<TerrainTile>::SLOTS;

/// The separation's columns, a plane each in order: line starts, points, tops,
/// width, kind, form, clear; each its length and its element's bytes, which
/// pxl-spec `separation::Room` is laid on (domain/layer/terrain/tile.rs `room_of`).
pub const SEPARATION_COLUMNS: [(usize, usize); 7] = [
    (MAX_LINES + 1, 4),
    (MAX_POINTS, 4),
    (MAX_POINTS, 2),
    (MAX_LINES, 1),
    (MAX_LINES, 1),
    (MAX_LINES, 1),
    (MAX_LINES, 1),
];

/// Separation column `k`'s bytes in a record.
const fn separation_column_bytes(k: usize) -> u32 {
    let (cnt, size) = SEPARATION_COLUMNS[k];
    arr_bytes(cnt, size)
}

/// Where separation column `k`'s plane starts; past the last, where the planes end.
const fn separation_plane(k: usize) -> u32 {
    let mut at = 0;
    let mut j = 0;
    while j < k {
        at += TILE_SLOTS * separation_column_bytes(j);
        j += 1;
    }
    at
}

const fn sum(bytes: &[u32]) -> u32 {
    let mut sum = 0;
    let mut k = 0;
    while k < bytes.len() {
        sum += bytes[k];
        k += 1;
    }
    sum
}

pub const SEPARATION_BYTES: u32 = separation_plane(SEPARATION_COLUMNS.len());

/// A tile's padded samples in a record, which a brick's fill reads after its
/// packet went.
const ELE_BYTES: u32 = arr_bytes(ELE_COLUMN, 2);
const ELE_AT: u32 = SEPARATION_BYTES;

/// One brick's cells whole: its columns, a word a cell (its ground's top),
/// then its levels, its cells as laid and its ways' tree's root (pxl-engine
/// `clipmap::BRICK_WORDS`).
pub const CELLS_BYTES: u32 = BRICK_WORDS * 4;
pub const CELLS_ARR_BYTES: u32 = arr_bytes(CELLS_BYTES as usize, 1);
const CELLS_AT: u32 = ELE_AT + TILE_SLOTS * ELE_BYTES;

/// One brick's four palette indices a cell, a word a cell, cut from its tile's
/// packet (clipmap.rs `cut`): what its fill lays its own off and the GPU
/// draws a hit by.
pub const PALETTE_ARR_BYTES: u32 = arr_bytes(BRICK_CELLS as usize, 4);
const PALETTE_AT: u32 = CELLS_AT + SLOTS * CELLS_ARR_BYTES;

/// The child groups a brick's tree is built in, past child group 0, and a word
/// each to walk them as they are laid in the pool (clipmap.rs `fill_stage`).
const BUILD_CHILD_GROUPS: usize = BRICK_CHILD_GROUPS as usize + 1;
pub const BUILD_WORDS: usize = BUILD_CHILD_GROUPS * CHILD_GROUP_WORDS as usize;

/// The room a fill works in (clipmap.rs `FillScratch`), an array each in order:
/// a tile's palette indices four a cell, a brick's samples, its lines, the
/// pairs `brick_lines` lays them with, its spans, its tree as it is built and
/// the walk that lays it.
pub const FILL_ARRAY_BYTES: [u32; 7] = [
    arr_bytes(PALETTE_COLUMN, 1),
    ELE_BYTES,
    arr_bytes(LINES_WORDS as usize, 4),
    arr_bytes(WAYS_BRICK_SEGMENTS as usize, 8),
    arr_bytes(SPANS_WORDS as usize, 4),
    arr_bytes(BUILD_WORDS, 4),
    arr_bytes(BUILD_CHILD_GROUPS, 4),
];

/// Where the fill's room starts, on a cache line.
const FILL_AT: u32 = arr_bytes(palette_off(SLOTS) as usize, 1);

pub const TERRAIN_BYTES: u32 = FILL_AT + sum(&FILL_ARRAY_BYTES);

/// Record `record`'s separation column `k`: where it starts, and its bytes.
pub const fn separation_column(record: u32, k: usize) -> (u32, u32) {
    let bytes = separation_column_bytes(k);
    (separation_plane(k) + record * bytes, bytes)
}

/// Record `record`'s samples.
pub const fn ele_off(record: u32) -> u32 {
    ELE_AT + record * ELE_BYTES
}

/// Slot `slot`'s cells (domain/layer/terrain/clipmap.rs `Slot`), `SLOTS` of them.
pub const fn cells_off(slot: u32) -> u32 {
    CELLS_AT + slot * CELLS_ARR_BYTES
}

/// Slot `slot`'s four palette indices a cell.
pub const fn palette_off(slot: u32) -> u32 {
    PALETTE_AT + slot * PALETTE_ARR_BYTES
}

pub const fn fill_off(idx: usize) -> u32 {
    let mut at = FILL_AT;
    let mut k = 0;
    while k < idx {
        at += FILL_ARRAY_BYTES[k];
        k += 1;
    }
    at
}
