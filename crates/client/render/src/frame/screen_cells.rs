// pxl-render/frame/screen_cells.rs

//! The camera grid's cells as the last published cast met them, one ray a
//! cell, in one lane at a fixed address the kernel writes once a cast and
//! the renderer reads in place (as `stage.rs`'s lane). A cell's four words,
//! the GPU's cast's as well (gpu-query-shader `word`): what the ray stopped
//! on, through which face and at which zoom; how far; the block's face bits
//! (pxl-portable `mask`); and the four looks a coarse ground block folds, or
//! in the steps view the nodes its descent asked.

use xpute_guest::global::Global;

/// Cells the lane holds: the camera grid's most (`pxl-kernel param/view.rs`
/// `GRID_CELLS_MAX`, held to it there).
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

pub struct ScreenCells {
    /// The cast's; a new one is uploaded.
    pub generation: u32,
    /// The grid's cells across and up the screen; cell `j · across[0] + i`.
    pub across: [u32; 2],
    /// Word 0 as above; word 1 the distance along the unit ray, f32 bits;
    /// word 2 the block's face bits; word 3 the four looks a coarse ground
    /// block folds, a byte each, `dx + 2·dy` (a finer one's own four times,
    /// none placed), or in the steps view the nodes the ray's descent asked
    /// the world.
    pub cells: [[u32; 4]; SCREEN_CELLS_MAX],
}

static CELLS: Global<ScreenCells> = Global::new();

pub fn screen_cells_mut() -> &'static mut ScreenCells {
    CELLS.get(|| ScreenCells {
        generation: 0,
        across: [0; 2],
        cells: [[0; 4]; SCREEN_CELLS_MAX],
    })
}
