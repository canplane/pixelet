// pxl-render/gpu/ray/raster.rs

//! The samples' stops handed to the rasterizer (pxl-spec-core `raster`,
//! pxl-shader raster.rs): after the samples, each stopped sample's box
//! written (`sample_box_cs`) and the boxes drawn at the rays' pixels
//! (raster.wgsl), the nearest face a pixel's ray enters kept with its
//! sample, in passes timed together (span `sraster`), which the cast's
//! pixels read (pxl-shader cast.rs `cast_cs`); then what lies about each
//! box's face asked once a box (`face_cs`, span `face`). Its args, boxes
//! and targets are written whole each frame before they are read.

use pxl_spec_core::{beam, eye, raster, sun};

use super::cast::HEAD_BINDING;

use crate::frame::shader::record::{compute, PIPELINE_RASTER, SHADER_FACE, SHADER_SAMPLE_BOX};
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
    /// The boxes' writing, the eye's and the sun's map's (pxl-shader
    /// raster.rs `sample_box_cs`), each over its own group.
    pub(super) boxes_built: Built,
    faces_built: Built,
    draw_built: Built,
    pub(super) args: u32,
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
    bound: [u32; 9],
    /// The sun's, made where its shadows are first cast (`sun_made`); and a
    /// pick of none, bound where it is not.
    pub(super) sun: SunRaster,
    blank_array: u32,
}

/// The sun's samples' boxes, written by the eye's pass (`boxes_built`) and
/// drawn from the sun by the same draw (pxl-spec-core `param::sun::SAMPLE`): its
/// boxes, room for a sample every `param::sun::SAMPLE` texels; their args; its
/// view, a uniform; its targets at the map's side, in the box's order; and
/// the draw's group.
#[derive(Default)]
pub(super) struct SunRaster {
    pub(super) boxes: u32,
    pub(super) args: u32,
    uniform: u32,
    picks: u32,
    picks_view: u32,
    picks_array: u32,
    depth: u32,
    depth_view: u32,
    group: u32,
}

impl Raster {
    /// Its pipelines and its args and uniform, made where the cast's are
    /// (`RaysPass::make`): its modules are a few kilobytes beside theirs.
    pub(super) fn new(g: &Gpu, pipelines: &mut Pipelines) -> Raster {
        let blank = g.texture(1, 1, 1, Format::r32uint, 1, texture_usage::TEXTURE_BINDING);
        Raster {
            boxes_built: pipelines.build_compute(g, SHADER_SAMPLE_BOX, &compute::SAMPLE_BOX_GROUP),
            faces_built: pipelines.build_compute(g, SHADER_FACE, &compute::FACE_GROUP),
            draw_built: pipelines.build(g, PIPELINE_RASTER, Format::r32uint),
            args: g.buffer(
                word_bytes(raster::ARGS_WORDS as usize),
                buffer_usage::STORAGE
                    | buffer_usage::COPY_DST
                    | buffer_usage::INDIRECT
                    | buffer_usage::COPY_SRC,
            ),
            boxes: 0,
            room: 0,
            uniform: g.buffer(
                word_bytes(UNIFORM),
                buffer_usage::UNIFORM | buffer_usage::COPY_DST,
            ),
            picks: 0,
            picks_view: 0,
            picks_array: 0,
            depth: 0,
            depth_view: 0,
            size: [0; 2],
            groups: [0; 3],
            bound: [0; 9],
            sun: SunRaster::default(),
            blank_array: g.view(blank, true, 0, 1, 0, 1),
        }
    }

    /// What a frame that casts needs, before its dispatches: the boxes made
    /// again where the `grid`'s samples outgrow them; its targets at the
    /// rays' `size`; the raster's view off the `eye`'s words; the args
    /// cleared; its groups over the cast's `eye`, `beams` and the eye's
    /// `head` (pxl-spec-core `sun::MOVED_SIZE`), and, for the faces'
    /// asking, the world's query's buffers (`world`: the window, the
    /// tiles' frames, the service area's index, the overwrites, the ways'
    /// pool).
    pub(super) fn prepare(
        &mut self,
        g: &Gpu,
        e: &[u32],
        size: [u32; 2],
        grid: &beam::Grid,
        [eye, beams, head]: [u32; 3],
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
            pxl_spec_core::param::raster::GROWN * pixel,
            w,
            h,
            reach,
            0.0,
            0.0,
            0.0,
            0.0,
        ];
        g.write_buffer(self.uniform, 0, bytes(&uniform));
        let mut args = [0u32; raster::ARGS_WORDS as usize];
        args[raster::ARGS_VERTICES as usize] = raster::VERTICES;
        args[raster::ARGS_FACES as usize + 1] = 1;
        args[raster::ARGS_FACES as usize + 2] = 1;
        g.write_buffer(self.args, 0, bytes(&args));
        let [window, ground, shards, overwrites, pool] = world;
        let bound = [
            eye, beams, head, self.boxes, window, ground, shards, overwrites, pool,
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
                        buffer_entry(HEAD_BINDING, head),
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
        let w = compute::SAMPLE_BOX_WORKGROUP;
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
        let pass = g.compute_pass(encoder, stamps[GpuSpanName::face as usize].pass());
        g.set_bind_group(pass, 0, self.groups[2], &[]);
        g.set_pipeline(pass, self.faces_built.pipeline);
        g.dispatch_indirect(pass, self.args, word_bytes(raster::ARGS_FACES as usize));
        g.end(pass);
    }
}

/// The raster's view's words (raster.wgsl `Raster`).
const UNIFORM: usize = 24;

impl Raster {
    /// The sun's buffers and its targets at the map's `size`, made once;
    /// whether they are.
    pub(super) fn sun_made(&mut self, g: &Gpu, size: u32) -> bool {
        let s = &mut self.sun;
        if s.group == 0 {
            let samples = (size / pxl_spec_core::param::sun::SAMPLE) * (size / pxl_spec_core::param::sun::SAMPLE);
            let storage = buffer_usage::STORAGE | buffer_usage::COPY_DST;
            s.boxes = g.buffer(word_bytes(raster::words(samples) as usize), storage);
            s.args = g.buffer(
                word_bytes(raster::ARGS_WORDS as usize),
                storage | buffer_usage::INDIRECT | buffer_usage::COPY_SRC,
            );
            s.uniform = g.buffer(
                word_bytes(UNIFORM),
                buffer_usage::UNIFORM | buffer_usage::COPY_DST,
            );
            let usage = texture_usage::RENDER_ATTACHMENT | texture_usage::TEXTURE_BINDING;
            s.picks = g.texture(size, size, 1, Format::r32uint, 1, usage);
            s.picks_view = g.view(s.picks, false, 0, 1, 0, 1);
            s.picks_array = g.view(s.picks, true, 0, 1, 0, 1);
            s.depth = g.texture(size, size, 1, Format::depth32float, 1, usage);
            s.depth_view = g.view(s.depth, false, 0, 1, 0, 1);
            s.group = g.bind_group(
                self.draw_built.layouts[0],
                &[buffer_entry(0, s.uniform), buffer_entry(1, s.boxes)],
            );
        }
        s.group != 0
    }

    /// The sun's picks as its cast reads them, an array of one layer; a
    /// pick of none where they are not made.
    pub(super) fn sun_picks(&self) -> u32 {
        if self.sun.group != 0 {
            self.sun.picks_array
        } else {
            self.blank_array
        }
    }

    /// The sun's view off the eye's words `e` (pxl-spec-core `sun::Sun`) for
    /// the map `size` a side, as `prepare` lays the eye's; the args cleared.
    pub(super) fn sun_prepare(&self, g: &Gpu, e: &[u32], size: u32) {
        let s = &self.sun;
        if s.group == 0 {
            return;
        }
        let words: &[u32; eye::SUN_WORDS] = e[eye::SUN..eye::SUN + eye::SUN_WORDS]
            .try_into()
            .unwrap_or(&[0; eye::SUN_WORDS]);
        let v = sun::Sun::of_words(words);
        let length = |a: [f32; 3]| libm::sqrtf(a[0] * a[0] + a[1] * a[1] + a[2] * a[2]);
        let unit = |a: [f32; 3]| {
            let l = length(a).max(f32::MIN_POSITIVE);
            [a[0] / l, a[1] / l, a[2] / l]
        };
        let (r, d) = (unit(v.across), unit(v.down));
        let up = [-d[0], -d[1], -d[2]];
        let turn = 36 * u32::from(raster::flipped(r, up, v.dir));
        let n = size as f32;
        let uniform = [
            r[0],
            r[1],
            r[2],
            length(v.across),
            d[0],
            d[1],
            d[2],
            length(v.down),
            v.dir[0],
            v.dir[1],
            v.dir[2],
            0.0,
            f32::from_bits(turn),
            f32::from_bits(1),
            0.0,
            0.0,
            pxl_spec_core::param::raster::GROWN * length(v.across),
            n,
            n,
            v.reach,
            // The boxes are laid from box texel (0, 0)'s start (pxl-shader
            // sun.rs `Map::laid`).
            0.0,
            0.0,
            0.0,
            0.0,
        ];
        g.write_buffer(s.uniform, 0, bytes(&uniform));
        let mut args = [0u32; raster::ARGS_WORDS as usize];
        args[raster::ARGS_VERTICES as usize] = raster::VERTICES;
        g.write_buffer(s.args, 0, bytes(&args));
    }

    /// The sun's boxes drawn into its targets as many as their args say,
    /// cleared first, timed in `stamp` (span `sun_raster`), after its boxes
    /// are written.
    pub(super) fn sun_draw(&self, g: &Gpu, encoder: u32, stamp: &mut Stamp) {
        let s = &self.sun;
        let pass = g.pass(
            encoder,
            &pass_desc(
                s.picks_view,
                Load::Clear,
                s.depth_view,
                Load::Clear,
                0.0,
                stamp.pass(),
            ),
        );
        g.set_pipeline(pass, self.draw_built.pipeline);
        g.set_bind_group(pass, 0, s.group, &[]);
        g.draw_indirect(pass, s.args, 0);
        g.end(pass);
    }
}
