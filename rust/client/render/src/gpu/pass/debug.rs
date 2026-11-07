// pxl-render/gpu/pass/debug.rs

//! The dev gizmos, over the finished world in passes of their own.

use crate::frame::draw::{uniform_of, PIPELINE_DEBUG};
use crate::frame::draw::{DrawStats, Draws};
use crate::frame::shader::record::{
    DEBUG_FACE_VERTEX_BYTES, DEBUG_VERTEX_BYTES, PIPELINE_DEBUG_FACE,
};
use crate::gpu::api::{buf_entry, bytes, pass_desc, Gpu, Load, RenderContext};
use crate::gpu::pass::model::{DrawUniforms, DRAW_BYTES};
use crate::gpu::pipeline::{Pipeline, Pipelines};
use crate::gpu::span_spec::GpuSpanName;
use crate::gpu::upload::Uploads;

/// Their faces go into the canvas's depth, cleared once what is drawn over
/// the world is done with it, and their lines are drawn against those faces alone, so a gizmo
/// hides gizmos and nothing of the world hides one. Each set takes its own
/// position, a draw's uniform a set.
pub struct DebugPass {
    pipeline: Pipeline,
    faces: Pipeline,
    frame_group: u32,
    uniforms: DrawUniforms,
}

impl DebugPass {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines, frame_buf: u32, align: u32) -> DebugPass {
        let pipeline = pipelines.build(g, PIPELINE_DEBUG, g.canvas());
        let faces = pipelines.build(g, PIPELINE_DEBUG_FACE, g.canvas());
        let frame_group = g.bind_group(pipeline.layouts[0], &[buf_entry(0, frame_buf)]);
        DebugPass {
            pipeline,
            faces,
            frame_group,
            uniforms: DrawUniforms::new(DRAW_BYTES, align),
        }
    }

    pub fn prepare(&mut self, g: &Gpu, draws: &Draws, uniforms: &[f32], moved: bool) {
        let sets = draws.of(PIPELINE_DEBUG);
        self.uniforms.write(
            g,
            self.pipeline.layouts[1],
            sets.len() as u32,
            sets,
            moved,
            |buf, d| {
                buf[..DRAW_BYTES as usize].copy_from_slice(bytes(uniform_of(uniforms, d)));
            },
        );
    }

    /// Its draws into `stats`, two triangles a segment, and the faces'.
    pub fn count(&self, draws: &Draws, stats: &mut DrawStats) {
        draws.count(PIPELINE_DEBUG, stats, |d| d[1] + d[2] / 3);
    }

    /// `depth` is the world's, which nothing reads after this in the frame;
    /// whether it was drawn into, so the next frame carries it whole
    /// (gpu/finish.rs).
    pub fn draw(
        &self,
        ctx: &mut RenderContext,
        canvas: u32,
        depth: u32,
        draws: &Draws,
        uploads: &Uploads,
    ) -> bool {
        let (g, encoder) = (ctx.g, ctx.encoder);
        let stamp = ctx.stamp(GpuSpanName::debug);
        let held = self
            .uniforms
            .slots()
            .filter(|_| draws.of(PIPELINE_DEBUG).len() > 0);
        if held.is_none() {
            return false;
        }
        let slots = held.as_ref();
        // Each set held: its uniform's slot, its line and its face vertices.
        let sets = || {
            draws.of(PIPELINE_DEBUG).enumerate().filter_map(|(k, d)| {
                let slots = slots?;
                let (buf, _, _) = uploads.get(d[0])?;
                (d[1] > 0).then_some((k, d, buf, d[1], d[2], slots))
            })
        };
        // With no faces the lines' pass clears the depth itself.
        let faced = sets().any(|s| s.4 > 0);
        if faced {
            let pass = g.pass(
                encoder,
                &pass_desc(0, Load::Clear, depth, Load::Clear, 0.0, stamp.pass()),
            );
            g.set_pipeline(pass, self.faces.id);
            g.set_bind_group(pass, 0, self.frame_group, &[]);
            for (k, _, buf, lines, faces, slots) in sets().filter(|s| s.4 > 0) {
                let (group, off) = slots.at(k as u32);
                g.set_bind_group(pass, 1, group, &[off]);
                g.set_vertex_buf(
                    pass,
                    0,
                    buf,
                    lines * DEBUG_VERTEX_BYTES,
                    faces * DEBUG_FACE_VERTEX_BYTES,
                );
                g.draw(pass, faces, 1, 0, 0);
            }
            g.end(pass);
        }
        // A set's faces hide only its own lines: the faced sets' lines over
        // their faces' depth, then the rest over none, so the pivot's frame
        // is whole under the coverage laid on the ground.
        let depth_load = if faced { Load::Load } else { Load::Clear };
        let mut pass = g.pass(
            encoder,
            &pass_desc(canvas, Load::Load, depth, depth_load, 0.0, stamp.pass()),
        );
        for faces in [true, false] {
            if faced && !faces {
                g.end(pass);
                pass = g.pass(
                    encoder,
                    &pass_desc(canvas, Load::Load, depth, Load::Clear, 0.0, stamp.pass()),
                );
            }
            g.set_pipeline(pass, self.pipeline.id);
            g.set_bind_group(pass, 0, self.frame_group, &[]);
            for (k, _, buf, lines, _, slots) in sets().filter(|s| (s.4 > 0) == faces) {
                let (group, off) = slots.at(k as u32);
                g.set_bind_group(pass, 1, group, &[off]);
                g.set_vertex_buf(pass, 0, buf, 0, lines * DEBUG_VERTEX_BYTES);
                // A segment an instance: its two vertices, read as one.
                g.draw(pass, 6, lines / 2, 0, 0);
            }
        }
        g.end(pass);
        true
    }
}
