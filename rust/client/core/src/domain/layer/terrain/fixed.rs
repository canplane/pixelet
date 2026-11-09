// pxl-core/domain/layer/terrain/fixed.rs

//! The layout of `.fixed.terrain`, the room every resident terrain tile and
//! every held brick lies in, and where each slot's bytes are. The room is
//! a sequence of planes. A plane is one array with an element for every tile
//! slot, or for every brick slot, end to end. The planes, in order: a tile's
//! seven separation columns and its samples; a brick's cells and its palette
//! of four indices per cell; and, once, the room a fill works in. The trees
//! of the bricks' ways live on the heap and grow as they are taken
//! (pool.rs).

use pxl_engine::clipmap::{lines_words, BRICK_CELLS, BRICK_WORDS, SPANS_WORDS};
use pxl_engine::way_tree::{BRICK_CHILD_GROUPS, CHILD_GROUP_WORDS};
use pxl_spec::bake::tile::terrain::ELE_COLUMN;

use crate::domain::layer::tile_cache::TileCache;
use pxl_engine::clipmap::SLOTS;
use pxl_spec::bake::tile::terrain::TerrainTile;
use xpute_core::mem::range::arr_bytes;
use xpute_core::mem::range::OwnedRange;

/// The `.fixed.terrain` section, which the boot hands in.
pub static RANGE: OwnedRange = OwnedRange::new();

const TILE_SLOTS: u32 = TileCache::<TerrainTile>::SLOTS;

/// The most lines and points a tile's slot holds. A tile past either is refused where it
/// is decoded (pxl-spec `separation::Table::into_room`), as a body too long for its kind is,
/// and is read as the tile over it.
pub const MAX_LINES: usize = 438;
pub const MAX_POINTS: usize = 4193;

/// The most of its tile's ways' entries a brick's fill holds, and so of its lines' segments
/// (`LINES_WORDS`). A brick past it is refused where its fill begins (clipmap.rs `begin`) and
/// reads the coarser clip level.
pub const WAYS_BRICK_SEGMENTS: u32 = 1065;

/// The words of a brick's lines, as the fill's room holds them (pxl-engine
/// `clipmap::lines_words`), each entry its own segment at its most.
pub const LINES_WORDS: u32 = lines_words(WAYS_BRICK_SEGMENTS, WAYS_BRICK_SEGMENTS);

/// The separation's columns, one plane each, in order: line starts, points,
/// tops, width, kind, form, clear. Each entry is the column's length and its
/// element's bytes. pxl-spec `separation::Room` is laid on these columns
/// (tile.rs `room_of`).
pub const SEPARATION_COLUMNS: [(usize, usize); 7] = [
    (MAX_LINES + 1, 4),
    (MAX_POINTS, 4),
    (MAX_POINTS, 2),
    (MAX_LINES, 1),
    (MAX_LINES, 1),
    (MAX_LINES, 1),
    (MAX_LINES, 1),
];

const fn separation_column_bytes(k: usize) -> u32 {
    let (cnt, size) = SEPARATION_COLUMNS[k];
    arr_bytes(cnt, size)
}

/// Where separation column `k`'s plane starts; for `k` past the last
/// column, where the planes end.
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

/// The bytes of a tile's padded samples in a slot. The slot keeps them
/// because a brick's fill reads them after the tile's packet is freed.
const ELE_BYTES: u32 = arr_bytes(ELE_COLUMN, 2);
const ELE_AT: u32 = SEPARATION_BYTES;

/// The bytes of one brick's cells: its columns, one word per cell holding
/// the top of its ground, then its levels, its cells as laid, and the root of
/// its ways' tree (pxl-engine `clipmap::BRICK_WORDS`).
pub const CELLS_BYTES: u32 = BRICK_WORDS * 4;
pub const CELLS_ARR_BYTES: u32 = arr_bytes(CELLS_BYTES as usize, 1);
const CELLS_AT: u32 = ELE_AT + TILE_SLOTS * ELE_BYTES;

/// The bytes of one brick's palette: four palette indices per cell, packed
/// in one word, cut from its tile's packet (clipmap.rs `cut`). The brick's
/// fill lays its own cells from them, and the GPU colors a hit by them.
pub const PALETTE_ARR_BYTES: u32 = arr_bytes(BRICK_CELLS as usize, 4);
const PALETTE_AT: u32 = CELLS_AT + SLOTS * CELLS_ARR_BYTES;

/// The child groups a brick's tree is built in, beyond child group 0, with
/// one word each for the walk that lays them into the pool (clipmap.rs
/// `fill_stage`).
const BUILD_CHILD_GROUPS: usize = BRICK_CHILD_GROUPS as usize + 1;
pub const BUILD_WORDS: usize = BUILD_CHILD_GROUPS * CHILD_GROUP_WORDS as usize;

/// The room a fill works in (clipmap.rs `FillScratch`), one array each, in
/// order: a brick's samples, its lines, the pairs `brick_lines` lays them
/// with, its spans, its tree as it is built, and the walk that lays the tree.
pub const FILL_ARRAY_BYTES: [u32; 6] = [
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

/// Tile slot `slot`'s separation column `k`: where it starts, and its bytes.
pub const fn separation_column(slot: u32, k: usize) -> (u32, u32) {
    let bytes = separation_column_bytes(k);
    (separation_plane(k) + slot * bytes, bytes)
}

/// Where tile slot `slot`'s samples start.
pub const fn ele_off(slot: u32) -> u32 {
    ELE_AT + slot * ELE_BYTES
}

/// Where brick slot `slot`'s cells start (clipmap.rs `Slot`); there are
/// `SLOTS` brick slots.
pub const fn cells_off(slot: u32) -> u32 {
    CELLS_AT + slot * CELLS_ARR_BYTES
}

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
