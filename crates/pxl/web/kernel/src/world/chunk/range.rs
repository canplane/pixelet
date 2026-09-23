// pxl-kernel/world/chunk/range.rs

//! What a view is measured from, and the page and chunk spans it is measured in.

use pxl_spec::bake::constant::{CELL_SHAMT, CHUNK_CELL_SPAN, PAGE_CELL_SPAN, U_PER_CELL};
use pxl_spec::space::indexer::L3_SHAMT;
use pxl_spec::space::pvf3::ivec::{self, IVec};

pub const PAGE_CHUNK_AXIS: u32 = PAGE_CELL_SPAN / CHUNK_CELL_SPAN;
pub const CHUNK_WORLD_SPAN: f64 = CHUNK_CELL_SPAN as f64 * U_PER_CELL;
/// One page's span in world units.
pub const PAGE_SPAN: f64 = PAGE_CELL_SPAN as f64 * U_PER_CELL;
/// A page's side as a shift: the L3 level's, less the cell's.
pub const PAGE_SHAMT: u32 = L3_SHAMT - CELL_SHAMT as u32;

const _: () = assert!(1 << PAGE_SHAMT == PAGE_CELL_SPAN, "the shift is the span's");
const CHUNK_ALIGN: i64 = !(CHUNK_CELL_SPAN as i64 - 1);

const _: () = assert!(PAGE_CHUNK_AXIS * CHUNK_CELL_SPAN == PAGE_CELL_SPAN, "render: an L3 page must contain a whole number of chunks");

/// The position a view's ranges are measured from: the view centre, snapped
/// down to a chunk.
///
/// Chunk-aligned and not page-aligned, which is the whole point. Keyed on the
/// page the centre happened to be in, every range stepped by a whole page as a
/// boundary was crossed, and a page's worth of terrain entered or left in one
/// frame (issue/20260912-what-to-render-when.md). Snapped rather than exact so
/// that moving within one chunk re-derives nothing.
pub fn range_anchor_of(center: IVec) -> IVec {
    ivec::gather(ivec::X(center) & CHUNK_ALIGN, ivec::Y(center) & CHUNK_ALIGN, 0)
}

#[cfg(test)]
#[path = "range.test.rs"]
mod test;

#[cfg(test)]
#[path = "view_range.test.rs"]
mod view_range_test;
