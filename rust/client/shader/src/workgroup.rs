// pxl-shader/workgroup.rs

//! The workgroup sides an entry's own code counts by, held once: the code
//! reads each constant, and an entry's `threads` names it, which
//! pxl-shader-macro `compute` writes as its value, rust-gpu's `threads`
//! taking literals alone. pxl-engine holds those the GPU's buffers lay
//! out by (`beam::HIT_LANES`, `raster::FACES_LANES`, `ray::MISS_LANES`);
//! the rest are here, read by pxl-shader-macro as its own module.

/// `cast_cs`'s workgroup a side, a square of pixels.
pub const CAST_SIDE: u32 = 8;

/// The lanes of a workgroup a brick: `way_tree_cs`'s, which a level's
/// scan ranks the flagged across, and `way_return_cs`'s.
pub const WAY_LANES: u32 = 64;

/// Lanes `decor_thin_cs`'s workgroup has: a clip level cell each.
pub const DECOR_CELL_LANES: u32 = 64;

/// Lanes a decor pass's workgroup has (`decor_cnt_cs`, `decor_compact_cs`).
pub const DECOR_RECORD_LANES: u32 = 64;
