//! The dev gizmos: line segments over the world, drawn on top of everything
//! — the frame's origins, page corners and the map centre that
//! module/world/marker/debug_layer.tsx lays out. The glue writes the
//! segments whenever what they mark moves; the core holds them as a block
//! and lists their draw each frame, placed against the frame's origin.

use crate::build::World;
use crate::draw::{DrawList, PIPELINE_DEBUG};

/// Floats a vertex takes: its position relative to the set's origin (3)
/// and its colour, linear (3). Two vertices a segment.
pub const DEBUG_VERTEX_FLOATS: usize = 6;

#[derive(Default)]
pub struct DebugLines {
    at: usize,
    bytes: u32,
    handle: u32,
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
        let need = (vertex_cnt as usize * DEBUG_VERTEX_FLOATS * 4).max(4) as u32;
        let pool = world.volumes.mesh_pool();
        if need > self.bytes {
            if self.bytes > 0 {
                draw.free(self.handle);
                world.pools[pool].release(self.at, self.bytes);
                self.bytes = 0;
                self.handle = 0;
            }
            let bytes = need.next_power_of_two();
            let Some(at) = world.pools[pool].acquire(bytes) else {
                self.vertex_cnt = 0;
                return;
            };
            self.at = at;
            self.bytes = bytes;
        }
        let mem = world.volumes.mem();
        core::ptr::copy_nonoverlapping(mem.add(lane), mem.add(self.at), vertex_cnt as usize * DEBUG_VERTEX_FLOATS * 4);
        self.vertex_cnt = vertex_cnt;
        self.origin = origin;
        if self.handle == 0 {
            self.handle = draw.upload(self.at as u32, self.bytes, 0, 0);
        } else {
            draw.rewrite(self.handle, self.at as u32, need);
        }
    }

    pub fn draw(&self, draw: &mut DrawList, origin: [f64; 3]) {
        if self.vertex_cnt == 0 || self.handle == 0 {
            return;
        }
        let t = [(self.origin[0] - origin[0]) as f32, (self.origin[1] - origin[1]) as f32, (self.origin[2] - origin[2]) as f32];
        draw.draw_as(PIPELINE_DEBUG, self.handle, self.vertex_cnt, 0, t, 0.0, 1.0);
    }
}
