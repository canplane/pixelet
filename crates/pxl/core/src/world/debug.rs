//! The dev gizmos: line segments over the world, drawn on top of everything
//! — the frame's origins, page corners and the map centre that
//! module/world/marker/debug_layer.tsx lays out. The glue writes the
//! segments whenever what they mark moves; the core holds them as a block
//! and lists their draw each frame, placed against the frame's origin.

use crate::render::draw::{DrawList, Held, PIPELINE_DEBUG};
use crate::world::build::World;

/// Floats a vertex takes: its position relative to the set's origin (3)
/// and its colour, linear (3). Two vertices a segment.
pub const DEBUG_VERTEX_FLOATS: usize = 6;

#[derive(Default)]
pub struct DebugLines {
    held: Held,
    vertex_cnt: u32,
    origin: [f64; 3],
}

impl DebugLines {
    /// Replaces the segments with `vertex_cnt` vertices from `lane`,
    /// relative to `origin` in units.
    ///
    /// # Safety
    /// `lane` addresses vertex_cnt * DEBUG_VERTEX_FLOATS f32s in the memory.
    pub unsafe fn set(&mut self, world: &mut World, draw: &mut DrawList, origin: [f64; 3], vertex_cnt: u32, lane: usize) {
        let len = (vertex_cnt as usize * DEBUG_VERTEX_FLOATS * 4) as u32;
        let written = self.held.write(draw, world.volumes.mem(), lane, len);
        self.vertex_cnt = if written { vertex_cnt } else { 0 };
        self.origin = origin;
    }

    pub fn draw(&self, draw: &mut DrawList, origin: [f64; 3]) {
        if self.vertex_cnt == 0 || self.held.handle == 0 {
            return;
        }
        let t = DrawList::translation(self.origin, origin);
        draw.draw_as(PIPELINE_DEBUG, self.held.handle, self.vertex_cnt, 0, t, 0.0, 1.0);
    }
}
