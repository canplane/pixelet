// pxl-kernel/scene/indicator/debug_lines.rs

//! Line segments for the dev gizmos, and the faces between them: a face hides
//! the gizmo lines behind it and draws nothing, so the gizmos occlude one
//! another and nothing else. A module built without the gizmos holds none.

use pxl_render::frame::draw::{DrawList, Held, PIPELINE_DEBUG};
use pxl_render::gpu::api::bytes;
use pxl_spec::space::coord::v3::V3;

/// Position relative to the set's origin (3), and linear color and its
/// opacity (4); two vertices a segment.
pub const DEBUG_VERTEX_FLOATS: usize = 7;

/// Position relative to the set's origin; three vertices a triangle.
pub const DEBUG_FACE_FLOATS: usize = 3;

#[derive(Default)]
pub struct DebugLines {
    held: Held,
    line_cnt: u32,
    face_cnt: u32,
    origin: V3,
}

impl DebugLines {
    /// `lane` holds its first `lines` floats as lines, DEBUG_VERTEX_FLOATS a
    /// vertex, then faces, DEBUG_FACE_FLOATS a vertex, all relative to `origin`.
    pub fn set(&mut self, draw: &mut DrawList, origin: V3, lane: &[f32], lines: usize) {
        let written = self.held.write(draw, bytes(lane));
        (self.line_cnt, self.face_cnt) = match written {
            true => ((lines / DEBUG_VERTEX_FLOATS) as u32, ((lane.len() - lines) / DEBUG_FACE_FLOATS) as u32),
            false => (0, 0),
        };
        self.origin = origin;
    }

    pub fn draw(&self, draw: &mut DrawList, eye: V3) {
        if self.line_cnt == 0 || self.held.handle == 0 {
            return;
        }
        let t = DrawList::translation(self.origin, eye);
        draw.draw_as(PIPELINE_DEBUG, self.held.handle, self.line_cnt, self.face_cnt, t, 0.0, 1.0);
    }
}
