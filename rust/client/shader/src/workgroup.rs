// pxl-shader/workgroup.rs

//! The workgroup sides an entry's own code counts by, held once: the code
//! reads each constant, and an entry's `threads` names it, which
//! pxl-shader-macro `compute` writes as its value, rust-gpu's `threads`
//! taking literals alone. pxl-engine holds those the GPU's buffers lay
//! out by (`beam::HIT_LANES`, `raster::FACES_LANES`, `ray::MISS_LANES`);
//! the rest are here, read by pxl-shader-macro as its own module.

/// `cast_cs`'s workgroup a side, a square of pixels.
pub const CAST_SIDE: u32 = 8;



