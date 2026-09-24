// pxl-kernel/world/chunk/range.rs

//! What a view is measured from.

use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pyramid::chunk_span_of;

/// A leaf chunk's side in units, as a mask: a chunk is a whole number of
/// units whatever a cell is.
const CHUNK_ALIGN: i64 = !(chunk_span_of(0) as i64 - 1);

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
