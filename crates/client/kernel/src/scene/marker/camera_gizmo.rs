// pxl-kernel/scene/marker/camera_gizmo.rs

//! The camera gizmo (the `camera_gizmo` setting): the camera grid's cells over
//! the screen, drawn per pixel by the renderer (`pxl-render
//! frame/shader/camera_grid.wgsl`), to judge by eye whether the chunks' cells
//! sit at its resolution. Listed is shown; it reads nothing but the frame.

use pxl_render::frame::draw::{DrawList, PIPELINE_CAMERA_GRID};

pub fn draw(draw: &mut DrawList, show: bool) {
    if show {
        draw.draw_as(PIPELINE_CAMERA_GRID, 0, 0, 0, [0.0; 3], 0.0, 1.0);
    }
}
