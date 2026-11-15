// pxl-core/domain/scene/gizmo/debug_line.rs

//! Line segments for the debug gizmos, and faces among them. A face draws
//! nothing itself but hides the gizmo lines behind it, so the gizmos occlude
//! one another and nothing else.

use pxl_render::frame::shader::record;
use pxl_render::frame::shader::record::PIPELINE_DEBUG;
use pxl_render::gpu::api::bytes;
use pxl_render::gpu::draw::{DrawId, DrawList, MirroredBlock, Position};
use pxl_spec::space::coord::v3::V3;

/// The floats of a line vertex: its position relative to the set's origin
/// (3), then its linear color and opacity (4). A segment takes two vertices.
/// The layout is the renderer's (pxl-render frame/shader/record.rs
/// `DEBUG_VERTEX_SZ`).
pub(crate) const DEBUG_VERTEX_FLOATS: usize = record::DEBUG_VERTEX_SZ as usize / 4;

/// The floats of a face vertex: its position relative to the set's origin. A
/// triangle takes three vertices.
pub(crate) const DEBUG_FACE_FLOATS: usize = 3;

#[derive(Default)]
pub(crate) struct DebugMesh {
    block: MirroredBlock,
    line_cnt: u32,
    face_cnt: u32,
    origin: V3,
    id: DrawId,
}

impl DebugMesh {
    /// Sets what is drawn. The first `lines` floats of `vertices` are line
    /// vertices, `DEBUG_VERTEX_FLOATS` each, and the rest are face vertices,
    /// `DEBUG_FACE_FLOATS` each, all relative to `origin`. When the block
    /// cannot be written, nothing is drawn.
    pub(crate) fn set(&mut self, draw: &mut DrawList, origin: V3, vertices: &[f32], lines: usize) {
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

    pub(crate) fn draw(&self, draw: &mut DrawList) {
        if self.line_cnt == 0 || self.block.handle == 0 {
            return;
        }
        let t = Position::of(self.origin);
        draw.draw_as(
            &self.id,
            PIPELINE_DEBUG,
            self.block.handle,
            self.line_cnt,
            self.face_cnt,
            t,
            None,
        );
    }
}
