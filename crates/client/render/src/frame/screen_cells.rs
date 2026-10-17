// pxl-render/frame/screen_cells.rs

//! The screen's rays as the GPU's cast leaves them (gpu-query-shader
//! `word`) and the screen pass reads them, a ray a drawn pixel, its words a
//! layer each of the world surface's `rays` (gpu/surface.rs): what the ray
//! stopped on, through which face and at which zoom; how far; the block's
//! face bits (pxl-portable `mask`); and the four looks a coarse ground block
//! folds, or in the steps view the nodes its descent asked.

/// A ray's words, the layers of the surface's `rays`.
pub const RAY_WORDS: u32 = 4;

/// Word 0's fields: the value, a ground look or a placed catalog value.
pub const VALUE_BITS: u32 = 16;
/// What the ray met: nothing, the ground, or what is placed.
pub const KIND_SHIFT: u32 = 16;
pub const KIND_NONE: u32 = 0;
pub const KIND_GROUND: u32 = 1;
pub const KIND_PLACED: u32 = 2;
/// The face it entered through: 2·axis, plus one where its outward normal
/// points down the axis.
pub const FACE_SHIFT: u32 = 18;
/// The zoom of the block it stopped on, six bits: up to the finest.
pub const ZOOM_SHIFT: u32 = 21;
