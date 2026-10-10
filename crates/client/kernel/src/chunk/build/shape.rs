// pxl-kernel/chunk/build/shape.rs

//! Which cells a chunk draws as held.

use crate::chunk::Chunk as PyramidChunk;
use pxl_spec::space::coord::zoom::Zoom;

/// Whether `c` draws what is held at `held` as it is, not folded into a
/// coarser cell: only there is it struck, hidden or cut by a preview.
pub fn draws_as_held(c: PyramidChunk, held: Zoom) -> bool {
    c.cell_zoom() >= held
}
