// pxl-core/domain/layer/terrain/fixed.rs

//! The layout of the terrain's fixed range (sys/range.rs), the room every resident terrain tile and
//! every held brick lies in, and where each slot's bytes are. The room is
//! a sequence of planes. A plane is one array with an element for every tile
//! slot, or for every brick slot, end to end. The planes, in order: a tile's
//! samples; a brick's cells and its folds, four asset ids per cell; and,
//! once, the room a fill works in. The ways live on the heap, in one pool
//! sized by what is resident (pool.rs).

use crate::domain::layer::terrain::tile::TerrainTile;
use pxl_engine::clipmap::{BRICK_WORDS, SPANS_WORDS};
use pxl_spec::bake::tile::terrain::ELE_COLUMN;
use pxl_spec::brick::BRICK_CELL_CNT;

use crate::domain::layer::tile_cache::TileCache;
use crate::sys::range::{OwnedRange, arr_sz};
use pxl_engine::clipmap::SLOT_CNT;

/// The terrain's range in `.fixed`, which the boot hands in.
pub(crate) static RANGE: OwnedRange = OwnedRange::new();

const TILE_SLOT_CNT: u32 = TileCache::<TerrainTile>::SLOT_CNT;

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
const ELE_SZ: u32 = arr_sz(ELE_COLUMN, 2);
const ELE_AT: u32 = 0;

/// The bytes of one brick's cells: its columns, one word per cell holding
/// the top of its ground, then its levels, its cells as laid, and the root
/// of its run of kept spans (pxl-engine `clipmap::BRICK_WORDS`).
const CELLS_SZ: u32 = BRICK_WORDS * 4;
pub(super) const CELLS_ARR_SZ: u32 = arr_sz(CELLS_SZ as usize, 1);
const CELLS_AT: u32 = ELE_AT + TILE_SLOT_CNT * ELE_SZ;

/// The bytes of one brick's folds: four asset ids per cell, packed
/// in one word, cut from its tile's packet (clipmap.rs `cut`). The brick's
/// fill lays its own cells from them, and the GPU colors a hit by them.
pub(super) const FOLDS_ARR_SZ: u32 = arr_sz(BRICK_CELL_CNT as usize, 4);
const FOLDS_AT: u32 = CELLS_AT + SLOT_CNT * CELLS_ARR_SZ;

/// The room a fill works in (clipmap.rs `FillScratch`), one array each, in
/// order: a brick's samples and its spans.
const FILL_ARRAY_SZ: [u32; 2] = [ELE_SZ, arr_sz(SPANS_WORDS as usize, 4)];

/// Where the fill's room starts, on a cache line.
const FILL_AT: u32 = arr_sz(folds_off(SLOT_CNT) as usize, 1);

pub(crate) const TERRAIN_SZ: u32 = FILL_AT + sum(&FILL_ARRAY_SZ);

/// Where tile slot `slot`'s samples start.
pub(super) const fn ele_off(slot: u32) -> u32 {
    ELE_AT + slot * ELE_SZ
}

/// Where brick slot `slot`'s cells start (clipmap.rs `Slot`); there are
/// `SLOT_CNT` brick slots.
pub(super) const fn cells_off(slot: u32) -> u32 {
    CELLS_AT + slot * CELLS_ARR_SZ
}

pub(super) const fn folds_off(slot: u32) -> u32 {
    FOLDS_AT + slot * FOLDS_ARR_SZ
}

pub(super) const fn fill_off(idx: usize) -> u32 {
    let mut at = FILL_AT;
    let mut k = 0;
    while k < idx {
        at += FILL_ARRAY_SZ[k];
        k += 1;
    }
    at
}
