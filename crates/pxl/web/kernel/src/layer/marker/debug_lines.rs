// pxl-kernel/layer/marker/debug_lines.rs

//! Line segments over the world, drawn on top of everything: what the dev
//! gizmos (frame_gizmo.rs) mark — the frame's origins, page corners and the
//! map center. The segments are written whenever what they mark moves; this
//! holds them as a block and lists their draw each frame, placed against the
//! frame's origin. A module built without the gizmos holds none.

use pxl_render::frame::draw::{DrawList, Held, PIPELINE_DEBUG};
use pxl_render::gpu::api::word_bytes;
use pxl_spec::space::v3::V3;

use crate::world::chunk::build::World;

/// Floats a vertex takes: its position relative to the set's origin (3)
/// and its color, linear (3). Two vertices a segment.
pub const DEBUG_VERTEX_FLOATS: usize = 6;

#[derive(Default)]
pub struct DebugLines {
    held: Held,
    vertex_cnt: u32,
    origin: V3,
}

impl DebugLines {
    /// Replaces the segments with `vertex_cnt` vertices from `lane`,
    /// relative to `origin` in units.
    ///
    /// # Safety
    /// `lane` addresses vertex_cnt * DEBUG_VERTEX_FLOATS f32s in the memory.
    pub unsafe fn set(&mut self, world: &mut World, draw: &mut DrawList, origin: V3, vertex_cnt: u32, lane: usize) {
        let len = word_bytes(vertex_cnt as usize * DEBUG_VERTEX_FLOATS);
        let written = self.held.write(draw, world.volumes.mem(), lane, len);
        self.vertex_cnt = if written { vertex_cnt } else { 0 };
        self.origin = origin;
    }

    pub fn draw(&self, draw: &mut DrawList, origin: V3) {
        if self.vertex_cnt == 0 || self.held.handle == 0 {
            return;
        }
        let t = DrawList::translation(self.origin, origin);
        draw.draw_as(PIPELINE_DEBUG, self.held.handle, self.vertex_cnt, 0, t, 0.0, 1.0);
    }
}
