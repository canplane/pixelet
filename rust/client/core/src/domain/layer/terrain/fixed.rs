// pxl-core/domain/layer/terrain/fixed.rs

//! The layout of `.fixed.terrain`, the room every resident terrain tile and
//! every held brick lies in, and where each slot's bytes are. The room is
//! a sequence of planes. A plane is one array with an element for every tile
//! slot, or for every brick slot, end to end. The planes, in order: a tile's
//! samples; a brick's cells and its palette of four indices per cell; and,
//! once, the room a fill works in. The ways live on the heap, in one pool
//! sized by what is resident (pool.rs).

use pxl_engine::clipmap::{BRICK_CELLS, BRICK_WORDS, SPANS_WORDS};
use pxl_spec::bake::tile::terrain::ELE_COLUMN;

use crate::domain::layer::tile_cache::TileCache;
use pxl_engine::clipmap::SLOTS;
use pxl_spec::bake::tile::terrain::TerrainTile;
use xpute_core::mem::range::arr_bytes;
use xpute_core::mem::range::OwnedRange;

/// The `.fixed.terrain` section, which the boot hands in.
pub static RANGE: OwnedRange = OwnedRange::new();

const TILE_SLOTS: u32 = TileCache::<TerrainTile>::SLOTS;

const fn sum(bytes: &[u32]) -> u32 {
    let mut sum = 0;
    let mut k = 0;
    while k < bytes.len() {
        sum += bytes[k];
        k += 1;
    }
    sum
}

/// The bytes of a tile's padded samples in a slot. The slot keeps them
/// because a brick's fill reads them after the tile's packet is freed.
const ELE_BYTES: u32 = arr_bytes(ELE_COLUMN, 2);
const ELE_AT: u32 = 0;

/// The bytes of one brick's cells: its columns, one word per cell holding
/// the top of its ground, then its levels, its cells as laid, and the root
/// of its run of kept spans (pxl-engine `clipmap::BRICK_WORDS`).
const CELLS_BYTES: u32 = BRICK_WORDS * 4;
pub(super) const CELLS_ARR_BYTES: u32 = arr_bytes(CELLS_BYTES as usize, 1);
const CELLS_AT: u32 = ELE_AT + TILE_SLOTS * ELE_BYTES;

/// The bytes of one brick's palette: four palette indices per cell, packed
/// in one word, cut from its tile's packet (clipmap.rs `cut`). The brick's
/// fill lays its own cells from them, and the GPU colors a hit by them.
pub(super) const PALETTE_ARR_BYTES: u32 = arr_bytes(BRICK_CELLS as usize, 4);
const PALETTE_AT: u32 = CELLS_AT + SLOTS * CELLS_ARR_BYTES;

/// The room a fill works in (clipmap.rs `FillScratch`), one array each, in
/// order: a brick's samples and its spans.
const FILL_ARRAY_BYTES: [u32; 2] = [ELE_BYTES, arr_bytes(SPANS_WORDS as usize, 4)];

/// Where the fill's room starts, on a cache line.
const FILL_AT: u32 = arr_bytes(palette_off(SLOTS) as usize, 1);

pub const TERRAIN_BYTES: u32 = FILL_AT + sum(&FILL_ARRAY_BYTES);

/// Where tile slot `slot`'s samples start.
pub(super) const fn ele_off(slot: u32) -> u32 {
    ELE_AT + slot * ELE_BYTES
}

/// Where brick slot `slot`'s cells start (clipmap.rs `Slot`); there are
/// `SLOTS` brick slots.
pub(super) const fn cells_off(slot: u32) -> u32 {
    CELLS_AT + slot * CELLS_ARR_BYTES
}

pub(super) const fn palette_off(slot: u32) -> u32 {
    PALETTE_AT + slot * PALETTE_ARR_BYTES
}

pub(super) const fn fill_off(idx: usize) -> u32 {
    let mut at = FILL_AT;
    let mut k = 0;
    while k < idx {
        at += FILL_ARRAY_BYTES[k];
        k += 1;
    }
    at
}
