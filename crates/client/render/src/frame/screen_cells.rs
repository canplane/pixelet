// pxl-render/frame/screen_cells.rs

//! The screen's rays' lane as the GPU's cast writes it (gpu-query-shader
//! `word`) and the screen pass reads it, a cell a ray. A cell's four words: what the ray stopped
//! on, through which face and at which zoom; how far; the block's face bits
//! (pxl-portable `mask`); and the four looks a coarse ground block folds, or
//! in the steps view the nodes its descent asked.

/// Cells the lane holds at first; it grows to the cast's.
pub const SCREEN_CELLS_MAX: usize = 10_000;

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
