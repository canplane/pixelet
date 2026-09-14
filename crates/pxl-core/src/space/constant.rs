//! The lattice's spans, @pxl/shared/bake/constant.ts's: a cell is a unit, a
//! chunk (and a band, and a chunk's bake) is 64 cells a side, a page is
//! 1024 — sixteen chunks. One source, so a span is not restated where it
//! is used.

/// bake/constant.ts's U_PER_CELL: CELL_SHAMT is 0, so a cell is a unit.
pub const U_PER_CELL: f64 = 1.0;
/// A chunk's side, in cells, and as a shift.
pub const CHUNK_SHAMT: u32 = 6;
pub const CHUNK_SPAN: u32 = 1 << CHUNK_SHAMT;
pub const CHUNK_CELLS: usize = (CHUNK_SPAN * CHUNK_SPAN) as usize;
/// A chunk's side, in units.
pub const CHUNK_WORLD_SPAN: f64 = CHUNK_SPAN as f64 * U_PER_CELL;
/// A page's side, in cells, as a shift, and in chunks.
pub const PAGE_SHAMT: u32 = 10;
pub const PAGE_CELL_SPAN: u32 = 1 << PAGE_SHAMT;
pub const PAGE_CHUNK_AXIS: u32 = PAGE_CELL_SPAN / CHUNK_SPAN;
/// A page's side, in units.
pub const PAGE_SPAN: f64 = PAGE_CELL_SPAN as f64 * U_PER_CELL;
