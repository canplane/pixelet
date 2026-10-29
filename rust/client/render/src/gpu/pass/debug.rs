// pxl-render/gpu/pass/debug.rs

//! The dev gizmos, over the finished world in passes of their own, and the
//! camera grid over all of it, off the frame's cell and the sky's ray.

use crate::frame::draw::{uniform_of, PIPELINE_CAMERA_GRID, PIPELINE_DEBUG};
use crate::frame::shader::record::{
    DEBUG_FACE_VERTEX_BYTES, DEBUG_VERTEX_BYTES, PIPELINE_DEBUG_FACE,
};
use crate::gpu::api::{buf_entry, bytes, pass_desc, Gpu, Load, Stamp};
use crate::gpu::pass::model::DrawUniforms;
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::upload::{Draws, Uploads};

/// Their faces go into the canvas's depth, cleared once what is drawn over
/// the world is done with it, and their lines are drawn against those faces alone, so a gizmo
/// hides gizmos and nothing of the world hides one. Each set takes its own
/// translation, a draw's uniform a set.
pub struct DebugPass {
    built: Built,
    faces: Built,
    frame_group: u32,
    uniforms: DrawUniforms,
    grid: Built,
    grid_group: u32,
}

impl DebugPass {
    pub fn new(
        g: &Gpu,
        pipelines: &mut Pipelines,
        frame_buf: u32,
        sky_buf: u32,
        align: u32,
    ) -> DebugPass {
        let built = pipelines.build(g, PIPELINE_DEBUG, g.canvas());
        let faces = pipelines.build(g, PIPELINE_DEBUG_FACE, g.canvas());
        let frame_group = g.bind_group(built.layouts[0], &[buf_entry(0, frame_buf)]);
        let grid = pipelines.build(g, PIPELINE_CAMERA_GRID, g.canvas());
        let grid_group = g.bind_group(
            grid.layouts[0],
            &[buf_entry(0, frame_buf), buf_entry(19, sky_buf)],
        );
        DebugPass {
            built,
            faces,
            frame_group,
            uniforms: DrawUniforms::new(16, align),
            grid,
            grid_group,
        }
    }

    pub fn prepare(&mut self, g: &Gpu, draws: &Draws, uniforms: &[f32], moved: bool) {
        let sets = draws.of(PIPELINE_DEBUG);
        self.uniforms.write(
            g,
            self.built.layouts[1],
            sets.len() as u32,
            sets,
            moved,
            |buf, d| {
                let u = uniform_of(uniforms, d);
                buf[..16].copy_from_slice(bytes(&[u[0], u[1], u[2], 0.0]));
            },
        );
    }

    /// `depth` is the world's, which nothing reads after this.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        g: &Gpu,
        encoder: u32,
        canvas: u32,
        depth: u32,
        draws: &Draws,
        uploads: &Uploads,
        stamp: &mut Stamp,
    ) {
        let grid = draws.of(PIPELINE_CAMERA_GRID).len() > 0;
        let held = self
            .uniforms
            .slots()
            .filter(|_| draws.of(PIPELINE_DEBUG).len() > 0);
        if held.is_none() && !grid {
            return;
        }
        let slots = held.as_ref();
        // Each set held: its uniform's slot, its line and its face vertices.
        let sets = || {
            draws.of(PIPELINE_DEBUG).enumerate().filter_map(|(k, d)| {
                let slots = slots?;
                let (buffer, _, _) = uploads.get(d[0])?;
                (d[1] > 0).then_some((k, d, buffer, d[1], d[2], slots))
            })
        };
        // With no faces the lines' pass clears the depth itself.
        let faced = sets().any(|s| s.4 > 0);
        if faced {
            let pass = g.pass(
                encoder,
                &pass_desc(0, Load::Clear, depth, Load::Clear, 0.0, stamp.pass()),
            );
            g.set_pipeline(pass, self.faces.pipeline);
            g.set_bind_group(pass, 0, self.frame_group, &[]);
            for (k, _, buffer, lines, faces, slots) in sets().filter(|s| s.4 > 0) {
                let (group, off) = slots.at(k as u32);
                g.set_bind_group(pass, 1, group, &[off]);
                g.set_vertex_buf(
                    pass,
                    0,
                    buffer,
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
            g.set_pipeline(pass, self.built.pipeline);
            g.set_bind_group(pass, 0, self.frame_group, &[]);
            for (k, _, buf, lines, _, slots) in sets().filter(|s| (s.4 > 0) == faces) {
                let (group, offset) = slots.at(k as u32);
                g.set_bind_group(pass, 1, group, &[offset]);
                g.set_vertex_buf(pass, 0, buf, 0, lines * DEBUG_VERTEX_BYTES);
                // A segment an instance: its two vertices, read as one.
                g.draw(pass, 6, lines / 2, 0, 0);
            }
        }
        if grid {
            g.set_pipeline(pass, self.grid.pipeline);
            g.set_bind_group(pass, 0, self.grid_group, &[]);
            g.draw(pass, 3, 1, 0, 0);
        }
        g.end(pass);
    }
}
