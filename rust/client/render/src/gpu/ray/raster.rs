// pxl-render/gpu/ray/raster.rs

//! The samples' hits handed to the rasterizer (pxl-spec-core `raster`,
//! pxl-shader raster.rs): after the samples, each stopped sample's box
//! written (`sample_box_cs`) and the boxes drawn at the rays' pixels
//! (raster.wgsl), the nearest face a pixel's ray enters kept with its
//! sample, in passes timed together (span `sraster`), which the cast's
//! pixels read (pxl-shader cast.rs `cast_cs`); then what lies about each
//! box's face asked once a box (`face_cs`, span `face`). Its targets are
//! written whole each frame before they are read: in a frame not drawn
//! whole, through a gate of their own, the depth cleared to the nearest and
//! the claimed tiles set back to the far plane (raster_gate.wgsl), so the
//! boxes pass the depth test, early, there alone, and the cast's pixels,
//! which pass the other tiles by, read nothing there. Its args, boxes and
//! faces each frame its samples are cast, and kept where only the pixels are
//! cast again over them.

use pxl_spec_core::{beam, eye, light, raster, ray};

use super::cast::HEAD_BINDING;

use crate::frame::shader::record::{
    compute, PIPELINE_RASTER, PIPELINE_RASTER_GATE, SHADER_FACE, SHADER_SAMPLE_BOX,
};
use crate::frame::shader::word::{BOX_VERTICES, RASTER_FLOATS};
use crate::gpu::api::{
    buf_entry, buf_range_entry, buffer_usage, bytes, pass_desc, release, texture_usage, word_bytes,
    Format, Gpu, Load, RenderContext,
};
use crate::gpu::pipeline::{Pipeline, Pipelines};
use crate::gpu::span_spec::GpuSpanName;
use pxl_spec_core::param::light::SAMPLE;
use pxl_spec_core::param::raster::GROWTH;

/// The raster's pipelines, made with the cast's; its args, made with them;
/// its boxes, room for `room` samples, made again where the grid's samples
/// outgrow them; the raster's view, a uniform; its targets at the rays'
/// size: a pixel's sample (r32uint, drawn into as 2D and read as a
/// one-layer array, `visibility`, the visibility buffer) and the depth it is
/// tested by; and the groups of the boxes' writing, their drawing and the
/// faces' asking, the boxes' and the missed pixels' hits', made again
/// exactly when what they bind is another (`bindings`).
pub(super) struct Raster {
    /// The boxes' writing, the eye's and the shadow map's (pxl-shader
    /// raster.rs `sample_box_cs`), each over its own group.
    pub(super) boxes_built: Pipeline,
    faces_built: Pipeline,
    draw_built: Pipeline,
    /// The gate the boxes are drawn through in a frame not drawn whole, and
    /// its group over the claimed tiles (`Resolve::tiles`).
    gate: Pipeline,
    gate_group: u32,
    gate_bindings: (u32, u32),
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
    groups: [u32; 4],
    bindings: [u32; 10],
    /// The light's, made where its shadows are first cast (`light_made`); and a
    /// visibility buffer naming no sample, bound where it is not.
    pub(super) light: LightRaster,
    blank_array: u32,
}

/// The light's samples' boxes, written by the eye's pass (`boxes_built`) and
/// drawn from the light by the same draw (pxl-spec-core `param::light::SAMPLE`): its
/// boxes, room for a sample every `param::light::SAMPLE` texels of the
/// widest level; its targets at each side a level has, in the box's order;
/// one level's at a time, the levels cast one after another, each with its
/// own args, view, group and the targets of its side (`LightRasterLevel`).
#[derive(Default)]
pub(super) struct LightRaster {
    pub(super) boxes: u32,
    targets: Vec<LightTarget>,
    pub(super) levels: Vec<LightRasterLevel>,
}

/// The light's raster's targets at one side: its visibility buffer, as the
/// draw writes it and as an array of one layer, as the slots read it, and
/// its depth.
struct LightTarget {
    size: u32,
    visibility_view: u32,
    visibility_array: u32,
    depth_view: u32,
}

/// A level's draw of the light's boxes: their args, its view, a uniform, the
/// draw's group, and which targets it draws into (`LightTarget`).
pub(super) struct LightRasterLevel {
    pub(super) args: u32,
    uniform: u32,
    group: u32,
    target: usize,
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
            gate: pipelines.build(g, PIPELINE_RASTER_GATE, Format::r32uint),
            gate_group: 0,
            gate_bindings: (0, 0),
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
            groups: [0; 4],
            bindings: [0; 10],
            light: LightRaster::default(),
            blank_array: g.view(blank, true, 0, 1, 0, 1),
        }
    }

    /// What a frame that casts needs, before its dispatches: the boxes made
    /// again where the `grid`'s samples outgrow them; its targets at the
    /// rays' `size`; the raster's view off the `eye`'s words; the args
    /// cleared where the samples are cast again (`samples_cast`), else the
    /// boxes they left kept; its groups over the cast's `eye`, `beams` and
    /// the eye's `head` (pxl-spec-core `light::HEAD_SIZE`), and, for the
    /// faces' asking, the world's query's buffers (the clip level, the tiles'
    /// frames, the service area's index, the overwrites, the ways' pool)
    /// and the walk again's words (`marks`), which hold the missed pixels'
    /// hits' args (`ask_hits`).
    pub(super) fn prepare(
        &mut self,
        g: &Gpu,
        e: &[u32],
        size: [u32; 2],
        grid: &beam::Grid,
        samples_cast: bool,
        [eye, beams, head, clipmap, ground, shards, overwrites, pool, marks]: [u32; 9],
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
        let far = f32::from_bits(e[eye::FAR]);
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
            GROWTH * px,
            w,
            h,
            far,
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
            args[raster::ARGS_ASKS as usize] = grid.asks_at;
            g.write_buf(self.args, 0, bytes(&args));
        }
        let bindings = [
            eye, beams, head, self.boxes, clipmap, ground, shards, overwrites, pool, marks,
        ];
        if bindings != self.bindings {
            for group in self.groups {
                release(group);
            }
            // The faces' pass over the boxes' asks, its args the raster's,
            // or over the missed pixels' hits', theirs in the pyramid's
            // head: one pass, bound to either's args.
            let faces = |args: u32, at: u32| {
                g.bind_group(
                    self.faces_built.layouts[0],
                    &[
                        buf_entry(0, eye),
                        buf_entry(1, clipmap),
                        buf_entry(2, ground),
                        buf_entry(5, shards),
                        buf_entry(6, overwrites),
                        buf_entry(7, pool),
                        buf_entry(8, beams),
                        buf_range_entry(16, args, at, word_bytes(raster::ARGS_WORDS as usize)),
                    ],
                )
            };
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
                faces(self.args, 0),
                faces(marks, word_bytes(ray::MISS_ARGS_ROOM as usize)),
            ];
            self.bindings = bindings;
        }
    }

    /// What lies about each missed pixel's hit's face asked once a square of
    /// them, in the cast's pass `pass` (ray/cast.rs): the faces' pass over
    /// the hits' asks, its args among the walk again's words (`marks`,
    /// pxl-spec-core `ray::MISS_ARGS_ROOM`) as the boxes' are in its own.
    pub(super) fn ask_hits(&self, g: &Gpu, pass: u32, marks: u32) {
        g.set_pipeline(pass, self.faces_built.id);
        g.set_bind_group(pass, 0, self.groups[3], &[]);
        let at = ray::MISS_ARGS_ROOM + raster::ARGS_FACES;
        g.dispatch_indirect(pass, marks, word_bytes(at as usize));
    }

    /// The boxes written off the samples, a thread a sample of `grid`, where
    /// they were cast again (`samples_cast`), else the last kept; and drawn
    /// into its targets as many as their args say, cleared first, through
    /// the gate in the `claimed` tiles where the frame is not drawn whole
    /// (`Resolve::tiles`, `cnt` of them): in passes of their own, timed
    /// together (span `sraster`).
    pub(super) fn draw(
        &mut self,
        ctx: &mut RenderContext,
        grid: &beam::Grid,
        samples_cast: bool,
        claimed: Option<((u32, u32), u32)>,
    ) {
        let (g, encoder) = (ctx.g, ctx.encoder);
        let stamp = ctx.stamp(GpuSpanName::sraster);
        if samples_cast {
            let pass = g.compute_pass(encoder, stamp.pass());
            g.set_bind_group(pass, 0, self.groups[0], &[]);
            g.set_pipeline(pass, self.boxes_built.id);
            let w = compute::SAMPLE_BOX_WORKGROUP;
            let [x, y] = grid.samples;
            g.dispatch(pass, x.div_ceil(w[0]), y.div_ceil(w[1]), 1);
            g.end(pass);
        }
        // Gated, the depth starts at the nearest, which no box passes, and
        // the gate sets the claimed tiles back to the far plane; else it
        // starts there.
        let pass = g.pass(
            encoder,
            &pass_desc(
                self.visibility_view,
                Load::Clear,
                self.depth_view,
                Load::Clear,
                if claimed.is_some() { 1.0 } else { 0.0 },
                stamp.pass(),
            ),
        );
        if let Some((tiles, cnt)) = claimed {
            if tiles != self.gate_bindings {
                release(self.gate_group);
                self.gate_group = g.bind_group(
                    self.gate.layouts[0],
                    &[buf_entry(30, tiles.0), buf_entry(31, tiles.1)],
                );
                self.gate_bindings = tiles;
            }
            g.set_pipeline(pass, self.gate.id);
            g.set_bind_group(pass, 0, self.gate_group, &[]);
            g.draw(pass, 6, cnt, 0, 0);
        }
        g.set_pipeline(pass, self.draw_built.id);
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
        g.set_pipeline(pass, self.faces_built.id);
        g.dispatch_indirect(pass, self.args, word_bytes(raster::ARGS_FACES as usize));
        g.end(pass);
    }
}

impl Raster {
    /// The light's buffers and its targets at each side a level has, level
    /// `l` `size(l)` a side, made once, and each level's args, view and
    /// group; whether they are.
    pub(super) fn light_made(&mut self, g: &Gpu, size: impl Fn(u32) -> u32) -> bool {
        let s = &mut self.light;
        if s.levels.is_empty() {
            let widest = (0..light::LEVELS).map(&size).max().unwrap_or(0);
            let samples = (widest / SAMPLE) * (widest / SAMPLE);
            let storage = buffer_usage::STORAGE | buffer_usage::COPY_DST;
            s.boxes = g.buf(word_bytes(raster::words(samples) as usize), storage);
            let usage = texture_usage::RENDER_ATTACHMENT | texture_usage::TEXTURE_BINDING;
            let boxes = s.boxes;
            for l in 0..light::LEVELS {
                let n = size(l);
                let target = match s.targets.iter().position(|t| t.size == n) {
                    Some(k) => k,
                    None => {
                        let visibility = g.texture(n, n, 1, Format::r32uint, 1, usage);
                        let depth = g.texture(n, n, 1, Format::depth32float, 1, usage);
                        s.targets.push(LightTarget {
                            size: n,
                            visibility_view: g.view(visibility, false, 0, 1, 0, 1),
                            visibility_array: g.view(visibility, true, 0, 1, 0, 1),
                            depth_view: g.view(depth, false, 0, 1, 0, 1),
                        });
                        s.targets.len() - 1
                    }
                };
                let args = g.buf(
                    word_bytes(raster::ARGS_WORDS as usize),
                    storage | buffer_usage::INDIRECT | buffer_usage::COPY_SRC,
                );
                let uniform = g.buf(
                    word_bytes(RASTER_FLOATS),
                    buffer_usage::UNIFORM | buffer_usage::COPY_DST,
                );
                let group = g.bind_group(
                    self.draw_built.layouts[0],
                    &[buf_entry(0, uniform), buf_entry(1, boxes)],
                );
                s.levels.push(LightRasterLevel {
                    args,
                    uniform,
                    group,
                    target,
                });
            }
        }
        !s.levels.is_empty()
    }

    /// Level `l`'s visibility buffer as its cast reads it, an array of one
    /// layer; one naming no sample where it is not made.
    pub(super) fn light_visibility(&self, l: u32) -> u32 {
        let s = &self.light;
        s.levels
            .get(l as usize)
            .map_or(self.blank_array, |v| s.targets[v.target].visibility_array)
    }

    /// Level `l`'s view off its words (pxl-spec-core `light::Light`) for
    /// a map `size` a side, as `prepare` lays the eye's; its args cleared.
    pub(super) fn light_prepare(&self, g: &Gpu, l: u32, words: &[u32; light::WORDS], size: u32) {
        let Some(s) = self.light.levels.get(l as usize) else {
            return;
        };
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
            GROWTH * length(v.across),
            n,
            n,
            v.t_max,
            // The boxes are laid from box texel (0, 0)'s start (pxl-shader
            // light.rs `Map::raster_layout`).
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

    /// Level `l`'s boxes drawn into the light's targets as many as their
    /// args say, cleared first, timed in the span `light_raster`, after its
    /// boxes are written.
    pub(super) fn light_draw(&self, ctx: &mut RenderContext, l: u32) {
        let (g, encoder) = (ctx.g, ctx.encoder);
        let stamp = ctx.stamp(GpuSpanName::light_raster);
        let v = &self.light.levels[l as usize];
        let t = &self.light.targets[v.target];
        let pass = g.pass(
            encoder,
            &pass_desc(
                t.visibility_view,
                Load::Clear,
                t.depth_view,
                Load::Clear,
                0.0,
                stamp.pass(),
            ),
        );
        g.set_pipeline(pass, self.draw_built.id);
        g.set_bind_group(pass, 0, v.group, &[]);
        g.draw_indirect(pass, v.args, 0);
        g.end(pass);
    }
}
