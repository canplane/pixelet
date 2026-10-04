// pxl-kernel/layer/marker/debug_lines.rs

//! Line segments drawn over everything, for the dev gizmos. A module built
//! without the gizmos holds none.

use pxl_render::frame::draw::{DrawList, Held, PIPELINE_DEBUG};
use pxl_render::gpu::api::bytes;
use pxl_spec::space::v3::V3;

/// Position relative to the set's origin (3), and linear color and its
/// opacity (4); two vertices a segment.
pub const DEBUG_VERTEX_FLOATS: usize = 7;

#[derive(Default)]
pub struct DebugLines {
    held: Held,
    vertex_cnt: u32,
    origin: V3,
}

impl DebugLines {
    /// `lane` holds DEBUG_VERTEX_FLOATS floats a vertex, relative to `origin`.
    pub fn set(&mut self, draw: &mut DrawList, origin: V3, lane: &[f32]) {
        let written = self.held.write(draw, bytes(lane));
        self.vertex_cnt = if written { (lane.len() / DEBUG_VERTEX_FLOATS) as u32 } else { 0 };
        self.origin = origin;
    }

    pub fn draw(&self, draw: &mut DrawList, eye: V3) {
        if self.vertex_cnt == 0 || self.held.handle == 0 {
            return;
        }
        let t = DrawList::translation(self.origin, eye);
        draw.draw_as(PIPELINE_DEBUG, self.held.handle, self.vertex_cnt, 0, t, 0.0, 1.0);
    }
}
