// pxl-render/gpu/pass/debug.rs

//! The debug gizmos. They are drawn onto the canvas after the world is
//! finished, in passes of their own, and the camera grid over all of them,
//! off the sky's ray.

use crate::frame::shader::record::{
    DEBUG_FACE_VERTEX_BYTES, DEBUG_VERTEX_BYTES, PIPELINE_DEBUG_FACE,
};
use crate::frame::shader::record::{PIPELINE_CAMERA_GRID, PIPELINE_DEBUG};
use crate::gpu::api::{buf_entry, bytes, pass_desc, Gpu, Load, RenderContext};
use crate::gpu::draw::uniform_of;
use crate::gpu::draw::{DrawStat, DrawTable};
use crate::gpu::pass::model::{UniformBuffer, DRAW_BYTES};
use crate::gpu::pipeline::{Pipeline, PipelineCache};
use crate::gpu::span_spec::GpuSpanName;
use crate::gpu::upload::UploadTable;

/// The gizmos draw their faces into the world's depth at the canvas's size,
/// cleared once everything drawn over the world is done with it. Their lines
/// are tested against those faces alone, so nothing of the world hides a
/// gizmo. Each set of lines and faces has its own position, in a draw
/// uniform of its own.
pub struct DebugPass {
    pipeline: Pipeline,
    face: Pipeline,
    frame_group: u32,
    uniforms: UniformBuffer,
    grid: Pipeline,
    grid_group: u32,
}

impl DebugPass {
    /// `sky_buf` holds the sky's words, whose ray the camera grid is aimed by.
    pub fn new(
        g: &Gpu,
        pipelines: &mut PipelineCache,
        frame_buf: u32,
        sky_buf: u32,
        align: u32,
    ) -> DebugPass {
        let pipeline = pipelines.build(g, PIPELINE_DEBUG, g.canvas());
        let face = pipelines.build(g, PIPELINE_DEBUG_FACE, g.canvas());
        let frame_group = g.bind_group(pipeline.layouts[0], &[buf_entry(0, frame_buf)]);
        let grid = pipelines.build(g, PIPELINE_CAMERA_GRID, g.canvas());
        let grid_group = g.bind_group(grid.layouts[0], &[buf_entry(19, sky_buf)]);
        DebugPass {
            pipeline,
            face,
            frame_group,
            uniforms: UniformBuffer::new(DRAW_BYTES, align),
            grid,
            grid_group,
        }
    }

    pub fn prepare(&mut self, g: &Gpu, draws: &DrawTable, uniforms: &[f32], changed: bool) {
        let sets = draws.of(PIPELINE_DEBUG);
        self.uniforms.write(
            g,
            self.pipeline.layouts[1],
            sets.len() as u32,
            sets,
            changed,
            |buf, d| {
                buf[..DRAW_BYTES as usize].copy_from_slice(bytes(uniform_of(uniforms, d)));
            },
        );
    }

    /// Adds the pass's draws to `stat`: two triangles a line segment, and the
    /// faces' triangles.
    pub fn count(&self, draws: &DrawTable, stat: &mut DrawStat) {
        draws.count(PIPELINE_DEBUG, stat, |d| d[1] + d[2] / 3);
    }

    /// Draws the gizmos onto `canvas`, and the camera grid over them where
    /// `grid`, and says whether it drew into `depth`. `depth` is the world's
    /// depth at the canvas's size, which nothing reads after this pass in the
    /// frame. When it was drawn into, the next frame finishes it whole again
    /// (gpu/finish.rs).
    pub fn draw(
        &self,
        ctx: &mut RenderContext,
        canvas: u32,
        depth: u32,
        draws: &DrawTable,
        uploads: &UploadTable,
        grid: bool,
    ) -> bool {
        let (g, encoder) = (ctx.g, ctx.encoder);
        let stamp = ctx.stamp(GpuSpanName::debug);
        let uniform_slots = self
            .uniforms
            .slots()
            .filter(|_| draws.of(PIPELINE_DEBUG).len() > 0);
        if uniform_slots.is_none() && !grid {
            return false;
        }
        let slots = uniform_slots.as_ref();
        // Each set with lines whose vertices are on the device, as (slot,
        // record, vertex buffer, line vertices, face vertices, slots).
        let sets = || {
            draws.of(PIPELINE_DEBUG).enumerate().filter_map(|(k, d)| {
                let slots = slots?;
                let (buf, _, _) = uploads.get(d[0])?;
                (d[1] > 0).then_some((k, d, buf, d[1], d[2], slots))
            })
        };
        // When no set has faces, the faces' pass is skipped and the lines'
        // pass clears the depth itself.
        let faced = sets().any(|s| s.4 > 0);
        if faced {
            let pass = g.pass(
                encoder,
                &pass_desc(0, Load::Clear, depth, Load::Clear, 0.0, stamp.pass()),
            );
            g.set_pipeline(pass, self.face.id);
            g.set_bind_group(pass, 0, self.frame_group, &[]);
            for (k, _, buf, line_cnt, face_cnt, slots) in sets().filter(|s| s.4 > 0) {
                let (group, off) = slots.at(k as u32);
                g.set_bind_group(pass, 1, group, &[off]);
                g.set_vertex_buf(
                    pass,
                    0,
                    buf,
                    line_cnt * DEBUG_VERTEX_BYTES,
                    face_cnt * DEBUG_FACE_VERTEX_BYTES,
                );
                g.draw(pass, face_cnt, 1, 0, 0);
            }
            g.end(pass);
        }
        // Faces hide only the lines of the sets that have faces. Those lines
        // are drawn over the faces' depth; then the depth is cleared and the
        // other sets' lines are drawn over none. The pivot's frame has no
        // faces, so it stays whole under the coverage laid on the ground.
        let depth_load = if faced { Load::Load } else { Load::Clear };
        let mut pass = g.pass(
            encoder,
            &pass_desc(canvas, Load::Load, depth, depth_load, 0.0, stamp.pass()),
        );
        for has_face in [true, false] {
            if faced && !has_face {
                g.end(pass);
                pass = g.pass(
                    encoder,
                    &pass_desc(canvas, Load::Load, depth, Load::Clear, 0.0, stamp.pass()),
                );
            }
            g.set_pipeline(pass, self.pipeline.id);
            g.set_bind_group(pass, 0, self.frame_group, &[]);
            for (k, _, buf, line_cnt, _, slots) in sets().filter(|s| (s.4 > 0) == has_face) {
                let (group, off) = slots.at(k as u32);
                g.set_bind_group(pass, 1, group, &[off]);
                g.set_vertex_buf(pass, 0, buf, 0, line_cnt * DEBUG_VERTEX_BYTES);
                // Each segment is one instance, and its two vertices are read
                // as one stride (record.rs `DEBUG_BUFS`). Six vertices make
                // the quad that draws it.
                g.draw(pass, 6, line_cnt / 2, 0, 0);
            }
        }
        if grid {
            g.set_pipeline(pass, self.grid.id);
            g.set_bind_group(pass, 0, self.grid_group, &[]);
            g.draw(pass, 3, 1, 0, 0);
        }
        g.end(pass);
        true
    }
}
