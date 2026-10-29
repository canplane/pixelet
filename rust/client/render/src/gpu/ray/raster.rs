// pxl-render/gpu/ray/raster.rs

//! The samples' stops handed to the rasterizer (pxl-spec-core `raster`,
//! pxl-shader raster.rs): after the samples, each stopped sample's box
//! written (`sample_box_cs`) and the boxes drawn at the rays' pixels
//! (raster.wgsl), the nearest face a pixel's ray enters kept with its
//! sample, in passes timed together (span `sraster`), which the cast's
//! pixels read (pxl-shader cast.rs `cast_cs`); then what lies about each
//! box's face asked once a box (`face_cs`, span `face`). Its targets are
//! written whole each frame before they are read; its args, boxes and faces
//! each frame its samples are cast, and kept where only the pixels are cast
//! again over them.

use pxl_spec_core::{beam, eye, light, raster};

use super::cast::HEAD_BINDING;

use crate::frame::shader::record::{compute, PIPELINE_RASTER, SHADER_FACE, SHADER_SAMPLE_BOX};
use crate::frame::shader::word::{BOX_VERTICES, RASTER_FLOATS};
use crate::gpu::api::{
    buf_entry, buffer_usage, bytes, pass_desc, release, texture_usage, word_bytes, Format, Gpu,
    Load, RenderContext,
};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::span_spec::GpuSpanName;
use crate::gpu::surface::Region;
use pxl_spec_core::param::light::SAMPLE;
use pxl_spec_core::param::raster::GROWN;

/// The raster's pipelines, made with the cast's; its args, made with them;
/// its boxes, room for `room` samples, made again where the grid's samples
/// outgrow them; the raster's view, a uniform; its targets at the rays'
/// size: a pixel's sample (r32uint, drawn into as 2D and read as a
/// one-layer array, `visibility`, the visibility buffer) and the depth it is
/// tested by; and the groups of the boxes' writing, their drawing and their
/// faces' asking, made again exactly when what they bind is another (`bound`).
pub(super) struct Raster {
    /// The boxes' writing, the eye's and the shadow map's (pxl-shader
    /// raster.rs `sample_box_cs`), each over its own group.
    pub(super) boxes_built: Built,
    faces_built: Built,
    draw_built: Built,
    pub(super) args: u32,
    boxes: u32,
    room: u32,
    uniform: u32,
    visibility: u32,
    visibility_view: u32,
    pub(super) visibility_array: u32,
    depth: u32,
    depth_view: u32,
    size: [u32; 2],
    groups: [u32; 3],
    bound: [u32; 9],
    /// The light's, made where its shadows are first cast (`light_made`); and a
    /// visibility buffer naming no sample, bound where it is not.
    pub(super) light: LightRaster,
    blank_array: u32,
}

/// The light's samples' boxes, written by the eye's pass (`boxes_built`) and
/// drawn from the light by the same draw (pxl-spec-core `param::light::SAMPLE`): its
/// boxes, room for a sample every `param::light::SAMPLE` texels; their args; its
/// view, a uniform; its targets at the map's side, in the box's order; and
/// the draw's group.
#[derive(Default)]
pub(super) struct LightRaster {
    pub(super) boxes: u32,
    pub(super) args: u32,
    uniform: u32,
    visibility: u32,
    visibility_view: u32,
    visibility_array: u32,
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
            args: g.buf(
                word_bytes(raster::ARGS_WORDS as usize),
                buffer_usage::STORAGE
                    | buffer_usage::COPY_DST
                    | buffer_usage::INDIRECT
                    | buffer_usage::COPY_SRC,
            ),
            boxes: 0,
            room: 0,
            uniform: g.buf(
                word_bytes(RASTER_FLOATS),
                buffer_usage::UNIFORM | buffer_usage::COPY_DST,
            ),
            visibility: 0,
            visibility_view: 0,
            visibility_array: 0,
            depth: 0,
            depth_view: 0,
            size: [0; 2],
            groups: [0; 3],
            bound: [0; 9],
            light: LightRaster::default(),
            blank_array: g.view(blank, true, 0, 1, 0, 1),
        }
    }

    /// What a frame that casts needs, before its dispatches: the boxes made
    /// again where the `grid`'s samples outgrow them; its targets at the
    /// rays' `size`; the raster's view off the `eye`'s words; the args
    /// cleared where the samples are cast again (`samples_cast`), else the
    /// boxes they left kept; its groups over the cast's `eye`, `beams` and
    /// the eye's `head` (pxl-spec-core `light::MOVED_SIZE`), and, for the
    /// faces' asking, the world's query's buffers (the window, the tiles'
    /// frames, the service area's index, the overwrites, the ways' pool).
    pub(super) fn prepare(
        &mut self,
        g: &Gpu,
        e: &[u32],
        size: [u32; 2],
        grid: &beam::Grid,
        samples_cast: bool,
        [eye, beams, head, window, ground, shards, overwrites, pool]: [u32; 8],
    ) {
        let samples = grid.samples[0] * grid.samples[1];
        if samples > self.room {
            release(self.boxes);
            self.boxes = g.buf(
                word_bytes(raster::words(samples) as usize),
                buffer_usage::STORAGE | buffer_usage::COPY_DST,
            );
            self.room = samples;
        }
        if size != self.size {
            for v in [
                self.visibility_view,
                self.visibility_array,
                self.visibility,
                self.depth_view,
                self.depth,
            ] {
                release(v);
            }
            let [w, h] = size;
            let usage = texture_usage::RENDER_ATTACHMENT | texture_usage::TEXTURE_BINDING;
            self.visibility = g.texture(w, h, 1, Format::r32uint, 1, usage);
            self.visibility_view = g.view(self.visibility, false, 0, 1, 0, 1);
            self.visibility_array = g.view(self.visibility, true, 0, 1, 0, 1);
            self.depth = g.texture(w, h, 1, Format::depth32float, 1, usage);
            self.depth_view = g.view(self.depth, false, 0, 1, 0, 1);
            self.size = size;
        }
        // The cast's view (pxl-shader cast.rs `px_tan`): its axes, the
        // screen's half-angles, the near plane, the angle a face grows by,
        // the drawn pixels and the rays' reach, each pixel's ray as the cast
        // forms it.
        let v = |at: usize| [e[at], e[at + 1], e[at + 2]].map(f32::from_bits);
        let (r, u, f) = (v(eye::RIGHT), v(eye::UP), v(eye::FORWARD));
        let tan = [e[eye::TAN], e[eye::TAN + 1]].map(f32::from_bits);
        let [w, h] = size.map(|s| s.max(1) as f32);
        let reach = f32::from_bits(e[eye::REACH]);
        let px = (2.0 * tan[0] / w).max(2.0 * tan[1] / h);
        let turn = BOX_VERTICES * u32::from(raster::is_flipped(r, u, f));
        let uniform: [f32; RASTER_FLOATS] = [
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
            GROWN * px,
            w,
            h,
            reach,
            f32::from_bits(e[eye::JITTER]),
            f32::from_bits(e[eye::JITTER + 1]),
            0.0,
            0.0,
        ];
        g.write_buf(self.uniform, 0, bytes(&uniform));
        if samples_cast {
            let mut args = [0u32; raster::ARGS_WORDS as usize];
            args[raster::ARGS_VERTICES as usize] = raster::VERTICES;
            args[raster::ARGS_FACES as usize + 1] = 1;
            args[raster::ARGS_FACES as usize + 2] = 1;
            g.write_buf(self.args, 0, bytes(&args));
        }
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
                        buf_entry(0, eye),
                        buf_entry(8, beams),
                        buf_entry(HEAD_BINDING, head),
                        buf_entry(14, self.boxes),
                        buf_entry(16, self.args),
                    ],
                ),
                g.bind_group(
                    self.draw_built.layouts[0],
                    &[buf_entry(0, self.uniform), buf_entry(1, self.boxes)],
                ),
                g.bind_group(
                    self.faces_built.layouts[0],
                    &[
                        buf_entry(0, eye),
                        buf_entry(1, window),
                        buf_entry(2, ground),
                        buf_entry(5, shards),
                        buf_entry(6, overwrites),
                        buf_entry(7, pool),
                        buf_entry(8, beams),
                        buf_entry(14, self.boxes),
                        buf_entry(16, self.args),
                    ],
                ),
            ];
            self.bound = bound;
        }
    }

    /// The boxes written off the samples, a thread a sample of `grid`, where
    /// they were cast again (`samples_cast`), else the last kept; and drawn
    /// into its targets as many as their args say, cleared first, within the
    /// `region` the frame casts where it casts one: in passes of their own,
    /// timed together (span `sraster`).
    pub(super) fn draw(
        &self,
        ctx: &mut RenderContext,
        grid: &beam::Grid,
        samples_cast: bool,
        region: Option<Region>,
    ) {
        let (g, encoder) = (ctx.g, ctx.encoder);
        let stamp = ctx.stamp(GpuSpanName::sraster);
        if samples_cast {
            let pass = g.compute_pass(encoder, stamp.pass());
            g.set_bind_group(pass, 0, self.groups[0], &[]);
            g.set_pipeline(pass, self.boxes_built.pipeline);
            let w = compute::SAMPLE_BOX_WORKGROUP;
            let [x, y] = grid.samples;
            g.dispatch(pass, x.div_ceil(w[0]), y.div_ceil(w[1]), 1);
            g.end(pass);
        }
        let pass = g.pass(
            encoder,
            &pass_desc(
                self.visibility_view,
                Load::Clear,
                self.depth_view,
                Load::Clear,
                0.0,
                stamp.pass(),
            ),
        );
        if let Some(r) = region {
            g.set_scissor(pass, r.rect());
        }
        g.set_pipeline(pass, self.draw_built.pipeline);
        g.set_bind_group(pass, 0, self.groups[1], &[]);
        g.draw_indirect(pass, self.args, 0);
        g.end(pass);
        // What lies about each box's face, before the pixels read it off
        // their visible sample: what the samples' last cast left, where they
        // stand.
        if !samples_cast {
            return;
        }
        let pass = g.compute_pass(encoder, ctx.stamp(GpuSpanName::face).pass());
        g.set_bind_group(pass, 0, self.groups[2], &[]);
        g.set_pipeline(pass, self.faces_built.pipeline);
        g.dispatch_indirect(pass, self.args, word_bytes(raster::ARGS_FACES as usize));
        g.end(pass);
    }
}

impl Raster {
    /// The light's buffers and its targets at the map's `size`, made once;
    /// whether they are.
    pub(super) fn light_made(&mut self, g: &Gpu, size: u32) -> bool {
        let s = &mut self.light;
        if s.group == 0 {
            let samples = (size / SAMPLE) * (size / SAMPLE);
            let storage = buffer_usage::STORAGE | buffer_usage::COPY_DST;
            s.boxes = g.buf(word_bytes(raster::words(samples) as usize), storage);
            s.args = g.buf(
                word_bytes(raster::ARGS_WORDS as usize),
                storage | buffer_usage::INDIRECT | buffer_usage::COPY_SRC,
            );
            s.uniform = g.buf(
                word_bytes(RASTER_FLOATS),
                buffer_usage::UNIFORM | buffer_usage::COPY_DST,
            );
            let usage = texture_usage::RENDER_ATTACHMENT | texture_usage::TEXTURE_BINDING;
            s.visibility = g.texture(size, size, 1, Format::r32uint, 1, usage);
            s.visibility_view = g.view(s.visibility, false, 0, 1, 0, 1);
            s.visibility_array = g.view(s.visibility, true, 0, 1, 0, 1);
            s.depth = g.texture(size, size, 1, Format::depth32float, 1, usage);
            s.depth_view = g.view(s.depth, false, 0, 1, 0, 1);
            s.group = g.bind_group(
                self.draw_built.layouts[0],
                &[buf_entry(0, s.uniform), buf_entry(1, s.boxes)],
            );
        }
        s.group != 0
    }

    /// The light's visibility buffer as its cast reads it, an array of one layer;
    /// one naming no sample where it is not made.
    pub(super) fn light_visibility(&self) -> u32 {
        if self.light.group != 0 {
            self.light.visibility_array
        } else {
            self.blank_array
        }
    }

    /// The light's view off the eye's words `e` (pxl-spec-core `light::Light`) for
    /// the map `size` a side, as `prepare` lays the eye's; the args cleared.
    pub(super) fn light_prepare(&self, g: &Gpu, e: &[u32], size: u32) {
        let s = &self.light;
        if s.group == 0 {
            return;
        }
        let words: &[u32; eye::LIGHT_WORDS] = e[eye::LIGHT..eye::LIGHT + eye::LIGHT_WORDS]
            .try_into()
            .unwrap_or(&[0; eye::LIGHT_WORDS]);
        let v = light::Light::of_words(words);
        let length = |a: [f32; 3]| libm::sqrtf(a[0] * a[0] + a[1] * a[1] + a[2] * a[2]);
        let unit = |a: [f32; 3]| {
            let l = length(a).max(f32::MIN_POSITIVE);
            [a[0] / l, a[1] / l, a[2] / l]
        };
        let (r, d) = (unit(v.across), unit(v.down));
        let up = [-d[0], -d[1], -d[2]];
        let turn = BOX_VERTICES * u32::from(raster::is_flipped(r, up, v.dir));
        let n = size as f32;
        let uniform: [f32; RASTER_FLOATS] = [
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
            GROWN * length(v.across),
            n,
            n,
            v.reach,
            // The boxes are laid from box texel (0, 0)'s start (pxl-shader
            // light.rs `Map::laid`).
            0.0,
            0.0,
            0.0,
            0.0,
        ];
        g.write_buf(s.uniform, 0, bytes(&uniform));
        let mut args = [0u32; raster::ARGS_WORDS as usize];
        args[raster::ARGS_VERTICES as usize] = raster::VERTICES;
        g.write_buf(s.args, 0, bytes(&args));
    }

    /// The light's boxes drawn into its targets as many as their args say,
    /// cleared first, timed in the span `light_raster`, after its boxes
    /// are written.
    pub(super) fn light_draw(&self, ctx: &mut RenderContext) {
        let (g, encoder) = (ctx.g, ctx.encoder);
        let stamp = ctx.stamp(GpuSpanName::light_raster);
        let s = &self.light;
        let pass = g.pass(
            encoder,
            &pass_desc(
                s.visibility_view,
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
