// pxl-kernel/domain/scene/gizmo/debug_line.rs

//! Line segments for the dev gizmos, and the faces between them: a face hides
//! the gizmo lines behind it and draws nothing, so the gizmos occlude one
//! another and nothing else. A module built without the gizmos holds none.

use pxl_render::frame::draw::{DrawList, MirroredBlock, Position, PIPELINE_DEBUG};
use pxl_render::frame::shader::record;
use pxl_render::gpu::api::bytes;
use pxl_spec::space::coord::v3::V3;

/// Position relative to the set's origin (3), and linear color and its
/// opacity (4); two vertices a segment: the renderer's vertex, read in its
/// words (pxl-render frame/shader/record.rs `DEBUG_VERTEX_BYTES`).
pub const DEBUG_VERTEX_FLOATS: usize = record::DEBUG_VERTEX_BYTES as usize / 4;

/// Position relative to the set's origin; three vertices a triangle.
pub const DEBUG_FACE_FLOATS: usize = 3;

#[derive(Default)]
pub struct DebugLines {
    block: MirroredBlock,
    line_cnt: u32,
    face_cnt: u32,
    origin: V3,
}

impl DebugLines {
    /// `vertices` holds its first `lines` floats as lines, DEBUG_VERTEX_FLOATS a
    /// vertex, then faces, DEBUG_FACE_FLOATS a vertex, all relative to `origin`.
    pub fn set(&mut self, draw: &mut DrawList, origin: V3, vertices: &[f32], lines: usize) {
        let written = self.block.write(draw, bytes(vertices));
        (self.line_cnt, self.face_cnt) = match written {
            true => (
                (lines / DEBUG_VERTEX_FLOATS) as u32,
                ((vertices.len() - lines) / DEBUG_FACE_FLOATS) as u32,
            ),
            false => (0, 0),
        };
        self.origin = origin;
    }

    pub fn draw(&self, draw: &mut DrawList) {
        if self.line_cnt == 0 || self.block.handle == 0 {
            return;
        }
        let t = Position::of(self.origin);
        draw.draw_as(
            PIPELINE_DEBUG,
            self.block.handle,
            self.line_cnt,
            self.face_cnt,
            t,
            0.0,
            1.0,
            None,
        );
    }
}
