//! The lattice's spans, read from pxl_shared's bake/constant.rs (the pair
//! of @pxl/shared/bake/constant.ts): a cell is a unit, a chunk (and a band,
//! and a chunk's bake) is 64 cells a side, a page is 1024 — sixteen chunks.
//! One source, so a span is not restated where it is used: the shifts are
//! the indexer's, and the derived forms the world reads are named here.

use pxl_shared::spec::space::indexer::{L2_SHAMT, L3_SHAMT};

pub use pxl_shared::bake::constant::{CHUNK_CELL_SPAN as CHUNK_SPAN, PAGE_CELL_SPAN, U_PER_CELL};

/// A chunk's side as a shift: the L2 level's, less the cell's.
pub const CHUNK_SHAMT: u32 = L2_SHAMT - pxl_shared::bake::constant::CELL_SHAMT as u32;
pub const CHUNK_CELLS: usize = (CHUNK_SPAN * CHUNK_SPAN) as usize;
/// A chunk's side, in units.
pub const CHUNK_WORLD_SPAN: f64 = CHUNK_SPAN as f64 * U_PER_CELL;
/// A page's side as a shift, and in chunks.
pub const PAGE_SHAMT: u32 = L3_SHAMT - pxl_shared::bake::constant::CELL_SHAMT as u32;
pub const PAGE_CHUNK_AXIS: u32 = PAGE_CELL_SPAN / CHUNK_SPAN;
/// A page's side, in units.
pub const PAGE_SPAN: f64 = PAGE_CELL_SPAN as f64 * U_PER_CELL;

const _: () = assert!(1 << CHUNK_SHAMT == CHUNK_SPAN && 1 << PAGE_SHAMT == PAGE_CELL_SPAN, "the shifts are the spans'");
