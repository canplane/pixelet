// pxl-render/gpu/ray/raster.rs

//! The samples' stops handed to the rasterizer (pxl-spec-core `raster`,
//! pxl-shader raster.rs): after the samples, each stopped sample's box
//! written (`boxes_cs`) and the boxes drawn at the rays' pixels
//! (raster.wgsl), the nearest face a pixel's ray enters kept with its
//! sample, in passes timed together (span `sraster`), which the cast's
//! pixels read (pxl-shader cast.rs `cast_cs`); then what lies about each
//! box's face asked once a box (`faces_cs`, span `faces`). Its args, boxes
//! and targets are written whole each frame before they are read.

use pxl_spec_core::{beam, eye, raster};

use crate::frame::shader::record::{compute, PIPELINE_RASTER, SHADER_BOXES, SHADER_FACES};
use crate::gpu::api::{
    buffer_entry, buffer_usage, bytes, pass_desc, release, texture_usage, word_bytes, Format, Gpu,
    Load, Stamp,
};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::span_spec::GpuSpanName;

/// The raster's pipelines, made with the cast's; its args, made with them;
/// its boxes, room for `room` samples, made again where the grid's samples
/// outgrow them; the raster's view, a uniform; its targets at the rays'
/// size: a pixel's sample (r32uint, drawn into as 2D and read as a
/// one-layer array, `picks`) and the depth it is tested by; and the groups
/// of the boxes' writing, their drawing and their faces' asking, made again
/// exactly when what they bind is another (`bound`).
pub(super) struct Raster {
    boxes_built: Built,
    faces_built: Built,
    draw_built: Built,
    args: u32,
    boxes: u32,
    room: u32,
    uniform: u32,
    picks: u32,
    picks_view: u32,
    pub(super) picks_array: u32,
    depth: u32,
    depth_view: u32,
    size: [u32; 2],
    groups: [u32; 3],
    bound: [u32; 8],
}

impl Raster {
    /// Its pipelines and its args and uniform, made where the cast's are
    /// (`RaysPass::make`): its modules are a few kilobytes beside theirs.
    pub(super) fn new(g: &Gpu, pipelines: &mut Pipelines) -> Raster {
        Raster {
            boxes_built: pipelines.build_compute(g, SHADER_BOXES, &compute::BOXES_GROUP),
            faces_built: pipelines.build_compute(g, SHADER_FACES, &compute::FACES_GROUP),
            draw_built: pipelines.build(g, PIPELINE_RASTER, Format::r32uint),
            args: g.buffer(
                word_bytes(raster::ARGS_WORDS as usize),
                buffer_usage::STORAGE | buffer_usage::COPY_DST | buffer_usage::INDIRECT,
            ),
            boxes: 0,
            room: 0,
            uniform: g.buffer(
                word_bytes(20),
                buffer_usage::UNIFORM | buffer_usage::COPY_DST,
            ),
            picks: 0,
            picks_view: 0,
            picks_array: 0,
            depth: 0,
            depth_view: 0,
            size: [0; 2],
            groups: [0; 3],
            bound: [0; 8],
        }
    }

    /// What a frame that casts needs, before its dispatches: the boxes made
    /// again where the `grid`'s samples outgrow them; its targets at the
    /// rays' `size`; the raster's view off the `eye`'s words; the args
    /// cleared; its groups over the cast's `eye` and `beams`, and, for the
    /// faces' asking, the world's query's buffers (`world`: the window, the
    /// tiles' frames, the service area's index, the overwrites, the ways'
    /// pool).
    pub(super) fn prepare(
        &mut self,
        g: &Gpu,
        e: &[u32],
        size: [u32; 2],
        grid: &beam::Grid,
        [eye, beams]: [u32; 2],
        world: [u32; 5],
    ) {
        let samples = grid.samples[0] * grid.samples[1];
        if samples > self.room {
            release(self.boxes);
            self.boxes = g.buffer(
                word_bytes(raster::words(samples) as usize),
                buffer_usage::STORAGE | buffer_usage::COPY_DST,
            );
            self.room = samples;
        }
        if size != self.size {
            for v in [
                self.picks_view,
                self.picks_array,
                self.picks,
                self.depth_view,
                self.depth,
            ] {
                release(v);
            }
            let [w, h] = size;
            let usage = texture_usage::RENDER_ATTACHMENT | texture_usage::TEXTURE_BINDING;
            self.picks = g.texture(w, h, 1, Format::r32uint, 1, usage);
            self.picks_view = g.view(self.picks, false, 0, 1, 0, 1);
            self.picks_array = g.view(self.picks, true, 0, 1, 0, 1);
            self.depth = g.texture(w, h, 1, Format::depth32float, 1, usage);
            self.depth_view = g.view(self.depth, false, 0, 1, 0, 1);
            self.size = size;
        }
        // The cast's view (pxl-shader cast.rs `pixel_tan`): its axes, the
        // screen's half-angles, the near plane, the angle a face grows by,
        // the drawn pixels and the rays' reach, each pixel's ray as the cast
        // forms it.
        let v = |at: usize| [e[at], e[at + 1], e[at + 2]].map(f32::from_bits);
        let (r, u, f) = (v(eye::RIGHT), v(eye::UP), v(eye::FORWARD));
        let tan = [e[eye::TAN], e[eye::TAN + 1]].map(f32::from_bits);
        let [w, h] = size.map(|s| s.max(1) as f32);
        let reach = f32::from_bits(e[eye::REACH]);
        let pixel = (2.0 * tan[0] / w).max(2.0 * tan[1] / h);
        let turn = 36 * u32::from(raster::flipped(r, u, f));
        let uniform = [
            r[0],
            r[1],
            r[2],
            tan[0],
            u[0],
            u[1],
            u[2],
            tan[1],
            f[0],
            f[1],
            f[2],
            raster::NEAR,
            f32::from_bits(turn),
            0.0,
            0.0,
            0.0,
            raster::GROWN * pixel,
            w,
            h,
            reach,
        ];
        g.write_buffer(self.uniform, 0, bytes(&uniform));
        let mut args = [0u32; raster::ARGS_WORDS as usize];
        args[raster::ARGS_VERTICES as usize] = raster::VERTICES;
        args[raster::ARGS_FACES as usize + 1] = 1;
        args[raster::ARGS_FACES as usize + 2] = 1;
        g.write_buffer(self.args, 0, bytes(&args));
        let [window, ground, shards, overwrites, pool] = world;
        let bound = [
            eye, beams, self.boxes, window, ground, shards, overwrites, pool,
        ];
        if bound != self.bound {
            for group in self.groups {
                release(group);
            }
            self.groups = [
                g.bind_group(
                    self.boxes_built.layouts[0],
                    &[
                        buffer_entry(0, eye),
                        buffer_entry(8, beams),
                        buffer_entry(14, self.boxes),
                        buffer_entry(16, self.args),
                    ],
                ),
                g.bind_group(
                    self.draw_built.layouts[0],
                    &[buffer_entry(0, self.uniform), buffer_entry(1, self.boxes)],
                ),
                g.bind_group(
                    self.faces_built.layouts[0],
                    &[
                        buffer_entry(0, eye),
                        buffer_entry(1, window),
                        buffer_entry(2, ground),
                        buffer_entry(5, shards),
                        buffer_entry(6, overwrites),
                        buffer_entry(7, pool),
                        buffer_entry(8, beams),
                        buffer_entry(14, self.boxes),
                        buffer_entry(16, self.args),
                    ],
                ),
            ];
            self.bound = bound;
        }
    }

    /// The boxes written off the samples, a thread a sample of `grid`, and
    /// drawn into its targets as many as their args say, cleared first: in
    /// passes of their own, timed together (span `sraster`).
    pub(super) fn draw(&self, g: &Gpu, encoder: u32, stamps: &mut [Stamp], grid: &beam::Grid) {
        let stamp = &mut stamps[GpuSpanName::sraster as usize];
        let pass = g.compute_pass(encoder, stamp.pass());
        g.set_bind_group(pass, 0, self.groups[0], &[]);
        g.set_pipeline(pass, self.boxes_built.pipeline);
        let w = compute::BOXES_WORKGROUP;
        let [x, y] = grid.samples;
        g.dispatch(pass, x.div_ceil(w[0]), y.div_ceil(w[1]), 1);
        g.end(pass);
        let pass = g.pass(
            encoder,
            &pass_desc(
                self.picks_view,
                Load::Clear,
                self.depth_view,
                Load::Clear,
                0.0,
                stamp.pass(),
            ),
        );
        g.set_pipeline(pass, self.draw_built.pipeline);
        g.set_bind_group(pass, 0, self.groups[1], &[]);
        g.draw_indirect(pass, self.args, 0);
        g.end(pass);
        // What lies about each box's face, before the pixels read it off
        // their pick's sample.
        let pass = g.compute_pass(encoder, stamps[GpuSpanName::faces as usize].pass());
        g.set_bind_group(pass, 0, self.groups[2], &[]);
        g.set_pipeline(pass, self.faces_built.pipeline);
        g.dispatch_indirect(pass, self.args, word_bytes(raster::ARGS_FACES as usize));
        g.end(pass);
    }
}
