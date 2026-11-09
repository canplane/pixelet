// pxl-render/ray/raster.rs

//! The raster that draws the node cache's drawable entries as boxes, so that each drawn pixel
//! keeps the voxel its ray enters first (pxl-engine `record`, `raster`). After the samples,
//! each sample's stop sets its face on its node's entry (sample_hit.wgsl). Every drawable
//! entry in view is then written as a box (record_draw.wgsl) and drawn at the rays' pixels
//! (raster.wgsl), and the depth test keeps the nearest face a pixel's ray enters. The pixels'
//! cast reads that voxel (cast.wgsl `cast_cs`). What lies about each new face is asked once a
//! face (face.wgsl). An entry's drawing is forgotten with the entry, by the node cache's rule
//! (forget.wgsl).
//!
//! The targets are written whole each frame before they are read. In a frame not drawn whole,
//! a gate of their own clears the depth to the nearest and sets the claimed tiles back to the
//! far plane (raster_mask.wgsl), so boxes pass the early depth test in those tiles alone; the
//! pixels' cast skips the other tiles and reads nothing there. The args and the boxes are
//! written again each frame the samples are cast, and kept where only the pixels are cast
//! again over them.

use crate::frame::shader::access::Group;
use crate::frame::shader::sequence;
use pxl_engine::{beam, eye, light, node_cache, raster, record};

use crate::frame::shader::group::{
    BEAMS, FACES_BINDS, FACE_GROUP, HEADER_BINDING, HIT_FACES_OF_BINDS, LIGHT_ARGS, LIGHT_LIST,
    LIGHT_RASTER_ARGS, LIGHT_RECEIVER_TILES, LIGHT_RECORD_BINDS, LIGHT_RECORD_HITS_BINDS,
    LIGHT_RECORD_HITS_GROUP, LIGHT_SAMPLE_HIT_GROUP, NODE_CACHE, RASTER_ARGS, RECORD_BINDS,
    RECORD_GROUP, RECORD_HITS_GROUP, RECORD_HITS_OF_BINDS, SAMPLE_HIT_BINDS, SAMPLE_HIT_GROUP,
};
use crate::frame::shader::record::{
    COMPUTE_FACE, COMPUTE_LIGHT_RECORD_DRAW, COMPUTE_LIGHT_RECORD_HITS, COMPUTE_LIGHT_SAMPLE_HIT,
    COMPUTE_RECORD_DRAW, COMPUTE_RECORD_FACES, COMPUTE_RECORD_HITS, COMPUTE_SAMPLE_HIT,
    PIPELINE_RASTER, PIPELINE_RASTER_MASK,
};
use crate::frame::shader::word::{BOX_VERTICES, RASTER_FLOATS, SQUARE_SIDE};
use crate::gpu::api::{
    buf_entry, buffer_usage, bytes, pass_desc, release, texture_usage, view_entry, word_bytes,
    Format, Gpu, Load, RenderContext,
};
use crate::gpu::pipeline::{Pipeline, PipelineCache};
use crate::gpu::span_spec::GpuSpanName;
use pxl_engine::param::raster::GROWTH;

/// The eye's raster. Its targets are at the rays' size: the visibility buffer, which holds each
/// pixel's voxel (r32uint), and the depth it is tested by. The visibility buffer is drawn into
/// as a 2D view and read as an array of one layer (`visibility_array`).
pub(super) struct Raster {
    /// Sets each sample's stop on its node's drawable entry (sample_hit.wgsl).
    pub(super) sample_hit: Pipeline,
    /// The passes that write drawable entries as boxes, a thread a slot of the node cache: the
    /// eye's for the entries in view, the light's for those on its box (record_draw.wgsl).
    record_draw_pipeline: Pipeline,
    light_record_draw: Pipeline,
    /// Writes the holes' stops into their drawable entries (record_hits.wgsl).
    record_hit: Pipeline,
    /// Puts what lies about each hole's face at its pixel, read off its entry (record_hits.wgsl
    /// `record_faces_cs`).
    face_put: Pipeline,
    /// Writes the light's holes' stops into their drawable entries (record_hits.wgsl over
    /// record_hits.light.wgsl).
    light_record_hit: Pipeline,
    /// Sets the shadow map's samples' stops, as `sample_hit` sets the eye's (sample_hit.wgsl over
    /// sample.light.wgsl). Its group binds the light's head as well.
    pub(super) light_sample_hit: Pipeline,
    face: Pipeline,
    draw_pipeline: Pipeline,
    /// Draws the gate the boxes pass through in a frame not drawn whole: it sets the claimed
    /// tiles' depth back to the far plane (raster_mask.wgsl).
    mask: Pipeline,
    pub(super) args: u32,
    /// The dispatch over the holes' stops, copied out of the node cache's head
    /// (`node_cache::HITS_ARGS`). A dispatch cannot read its args from a buffer it binds
    /// writable, and the stops bind the node cache writable, so the args take a buffer of
    /// their own.
    hits: u32,
    draws: u32,
    uniform: u32,
    visibility: u32,
    visibility_view: u32,
    pub(super) visibility_array: u32,
    depth: u32,
    depth_view: u32,
    size: [u32; 2],
    /// The groups, by index: the samples' stops, the boxes' draw, the samples' faces' asks, the
    /// record's draw (made by `record_group`), the holes' faces' asks, and the holes' stops.
    groups: [u32; 6],
    bindings: [u32; 9],
    /// What the record's draw group binds (`record_group`): the eye and the node cache, then
    /// the light's head, list, args and stood list's args. The eye's draw needs the light's
    /// buffers to mark the tiles its boxes stand on (record_draw.wgsl `record_draw_cs`).
    record_bufs: [u32; 6],
    /// The light's raster, which exists while shadows are on (`light_made`, `light_release`).
    pub(super) light: LightRaster,
}

/// The light's raster over the same drawable entries (pxl-engine `record`). The light's
/// samples and holes set their stops on the entries as the eye's do. Its draw writes a box for
/// each entry on the light's box (record_draw.wgsl `light_record_draw_cs`) and draws them from
/// the light with the eye's raster pipeline. Its targets are the map's size, laid out as the
/// light's box is.
#[derive(Default)]
pub(super) struct LightRaster {
    draws: u32,
    pub(super) args: u32,
    /// The dispatch over its holes' stops, in a buffer of its own as the eye's is
    /// (`Raster::hits`).
    hits: u32,
    uniform: u32,
    visibility: u32,
    visibility_view: u32,
    pub(super) visibility_array: u32,
    depth: u32,
    depth_view: u32,
    group: u32,
    record_group: u32,
    record_bufs: [u32; 7],
    /// The group of its holes' stops (`LIGHT_RECORD_HITS_GROUP`), made again with the record's
    /// draw group.
    record_hits_group: u32,
}

impl Raster {
    pub(super) fn new(g: &Gpu, pipelines: &mut PipelineCache) -> Raster {
        Raster {
            sample_hit: pipelines.build_compute(g, COMPUTE_SAMPLE_HIT, &SAMPLE_HIT_GROUP),
            light_sample_hit: pipelines.build_compute(
                g,
                COMPUTE_LIGHT_SAMPLE_HIT,
                &LIGHT_SAMPLE_HIT_GROUP,
            ),
            record_draw_pipeline: pipelines.build_compute(g, COMPUTE_RECORD_DRAW, &RECORD_GROUP),
            light_record_draw: pipelines.build_compute(g, COMPUTE_LIGHT_RECORD_DRAW, &RECORD_GROUP),
            record_hit: pipelines.build_compute(g, COMPUTE_RECORD_HITS, &RECORD_HITS_GROUP),
            face_put: pipelines.build_compute(g, COMPUTE_RECORD_FACES, &RECORD_HITS_GROUP),
            light_record_hit: pipelines.build_compute(
                g,
                COMPUTE_LIGHT_RECORD_HITS,
                &LIGHT_RECORD_HITS_GROUP,
            ),
            face: pipelines.build_compute(g, COMPUTE_FACE, &FACE_GROUP),
            draw_pipeline: pipelines.build(g, PIPELINE_RASTER, Format::r32uint),
            mask: pipelines.build(g, PIPELINE_RASTER_MASK, Format::r32uint),
            args: g.buf(
                word_bytes(raster::WORDS as usize),
                buffer_usage::STORAGE
                    | buffer_usage::COPY_DST
                    | buffer_usage::INDIRECT
                    | buffer_usage::COPY_SRC,
            ),
            hits: g.buf(
                word_bytes(3),
                buffer_usage::COPY_DST | buffer_usage::INDIRECT,
            ),
            draws: g.buf(
                word_bytes(record::EYE_DRAWS as usize),
                buffer_usage::STORAGE | buffer_usage::COPY_DST,
            ),
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
            groups: [0; 6],
            bindings: [0; 9],
            record_bufs: [0; 6],
            light: LightRaster::default(),
        }
    }

    /// Readies the raster for a frame that casts the pixels, before its dispatches. It makes
    /// the targets again where the rays' `size` changed, and writes the raster's view off the
    /// eye's words `e`. Where the samples are cast again (`samples_cast`), it clears the args;
    /// otherwise the boxes the last cast left are kept. The groups bind the cast's eye, beams
    /// and node cache, the world query's buffers the faces' asks read, and the rays, at whose
    /// pixels the holes' faces are put (`record_hits`).
    pub(super) fn prepare(
        &mut self,
        g: &Gpu,
        e: &[u32],
        size: [u32; 2],
        grid: &beam::Grid,
        samples_cast: bool,
        [eye, beams, node_cache, clipmap, ground, shards, objects, pool, rays]: [u32; 9],
    ) {
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
        // The raster's view forms each pixel's ray as the cast does (grid.wgsl `px_tan`): the
        // eye's axes and the screen's half-angles, the near plane, the angle a face grows by,
        // the drawn pixels, the rays' reach, the jitter and the eye's origin.
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
            f32::from_bits(e[eye::ORIGIN]),
            f32::from_bits(e[eye::ORIGIN + 1]),
            f32::from_bits(e[eye::ORIGIN + 2]),
            0.0,
        ];
        g.write_buf(self.uniform, 0, bytes(&uniform));
        if samples_cast {
            // No box is drawn and no face asked yet. The faces' dispatch lies at
            // `raster::ASKED_AT`, and its asks start where the grid lays them
            // (`beam::Grid::asks_at`).
            let mut args = [0u32; (raster::ASKED_AT + raster::ARGS_WORDS) as usize];
            args[..raster::ARGS_WORDS as usize].copy_from_slice(&draw_args());
            let asked = raster::ASKED_AT as usize;
            args[asked + raster::ARGS_FACES as usize + 1] = 1;
            args[asked + raster::ARGS_FACES as usize + 2] = 1;
            args[asked + raster::ARGS_ASKS as usize] = grid.asks_at;
            g.write_buf(self.args, sequence::OPEN_RASTER_ARGS.to_at(), bytes(&args));
        }
        // The holes' stops have opened no face ask yet. They run every frame the pixels are
        // cast, so this is written in every one; their faces' dispatch lies at
        // `raster::HIT_ASKED_AT`.
        let mut hit_args = [0u32; raster::ARGS_WORDS as usize];
        hit_args[raster::ARGS_FACES as usize + 1] = 1;
        hit_args[raster::ARGS_FACES as usize + 2] = 1;
        hit_args[raster::ARGS_ASKS as usize] = grid.hit_asks_at;
        g.write_buf(
            self.args,
            sequence::OPEN_RASTER_HIT_ASKS.to_at(),
            bytes(&hit_args),
        );
        let bindings = [
            eye, beams, node_cache, clipmap, ground, shards, objects, pool, rays,
        ];
        if bindings != self.bindings {
            for group in self.groups {
                release(group);
            }
            let buf = |b| match b {
                BEAMS => beams,
                NODE_CACHE => node_cache,
                RASTER_ARGS => self.args,
                _ => 0,
            };
            // One faces' pass serves the samples' stops and the holes', each through a group
            // bound to its own asks.
            let faces = |table: &Group| {
                let world = [
                    buf_entry(0, eye),
                    buf_entry(1, clipmap),
                    buf_entry(2, ground),
                    buf_entry(5, shards),
                    buf_entry(6, objects),
                    buf_entry(7, pool),
                ];
                g.frame_group(self.face.layouts[0], table, buf, &world)
            };
            let eye_only = [buf_entry(0, eye)];
            let hits = [buf_entry(0, eye), view_entry(3, rays)];
            self.groups = [
                g.frame_group(
                    self.sample_hit.layouts[0],
                    &SAMPLE_HIT_BINDS,
                    buf,
                    &eye_only,
                ),
                g.bind_group(
                    self.draw_pipeline.layouts[0],
                    &[
                        buf_entry(1, self.uniform),
                        buf_entry(2, self.draws),
                        buf_entry(9, node_cache),
                    ],
                ),
                faces(&FACES_BINDS),
                // The record's draw group binds the light's buffers too, so `record_group`
                // makes it.
                0,
                faces(&HIT_FACES_OF_BINDS),
                g.frame_group(
                    self.record_hit.layouts[0],
                    &RECORD_HITS_OF_BINDS,
                    buf,
                    &hits,
                ),
            ];
            self.bindings = bindings;
            self.record_bufs = [0; 6];
        }
    }

    /// Makes the record's draw group (`RECORD_GROUP`) again where what it binds changed: the
    /// eye and the node cache the raster binds, and `light`, the light's head, list, args and
    /// stood list's args (light_receiver_tile.wgsl), by which the draw marks the tiles its
    /// boxes stand on. The cast calls it every frame it prepares, as the light's buffers are
    /// made and released apart from the raster's.
    pub(super) fn record_group(&mut self, g: &Gpu, light: [u32; 4]) {
        let [eye, _, node_cache, ..] = self.bindings;
        let [header, list, args, receiver_tiles] = light;
        let bound = [eye, node_cache, header, list, args, receiver_tiles];
        if eye == 0 || bound == self.record_bufs {
            return;
        }
        release(self.groups[3]);
        let buf = |b| match b {
            NODE_CACHE => node_cache,
            RASTER_ARGS => self.args,
            LIGHT_LIST => list,
            LIGHT_ARGS => args,
            LIGHT_RECEIVER_TILES => receiver_tiles,
            _ => 0,
        };
        let rest = [
            buf_entry(0, eye),
            buf_entry(14, self.draws),
            buf_entry(HEADER_BINDING, header),
        ];
        self.groups[3] = g.frame_group(
            self.record_draw_pipeline.layouts[0],
            &RECORD_BINDS,
            buf,
            &rest,
        );
        self.record_bufs = bound;
    }

    /// Records the holes' stops once the holes' queue is done (ray/cast.rs). Their dispatch is
    /// copied out of the node cache's head first (`hits`). Then, in one pass timed in `span`,
    /// the stops are written into their entries, the faces they opened are asked, and each
    /// stop's face word is put at its pixel off its entry.
    pub(super) fn record_hits(&self, ctx: &mut RenderContext, node_cache: u32, span: GpuSpanName) {
        let (g, encoder) = (ctx.g, ctx.encoder);
        g.copy_buf(
            encoder,
            (node_cache, sequence::RECORD_HITS_DISPATCH.from_at()),
            (self.hits, sequence::RECORD_HITS_DISPATCH.to_at()),
            word_bytes(3),
        );
        let pass = g.compute_pass(encoder, ctx.stamp(span).pass());
        g.set_pipeline(pass, self.record_hit.id);
        g.set_bind_group(pass, 0, self.groups[5], &[]);
        g.dispatch_indirect(pass, self.hits, sequence::RECORD_HITS.at());
        g.set_pipeline(pass, self.face.id);
        g.set_bind_group(pass, 0, self.groups[4], &[]);
        g.dispatch_indirect(pass, self.args, sequence::HIT_FACES.at());
        g.set_pipeline(pass, self.face_put.id);
        g.set_bind_group(pass, 0, self.groups[5], &[]);
        g.dispatch_indirect(pass, self.hits, sequence::HIT_FACES_PUT.at());
        g.end(pass);
    }

    /// Draws the raster. Where the samples were cast again (`samples_cast`), their stops are set
    /// on the entries first. Where `draws_record`, the entries in view are written as boxes
    /// again; otherwise the boxes the last draw wrote are kept. The boxes are then drawn into
    /// cleared targets, through the gate in the `claimed` tiles where the frame is not drawn
    /// whole.
    pub(super) fn draw(
        &self,
        ctx: &mut RenderContext,
        grid: &beam::Grid,
        (samples_cast, draws_record): (bool, bool),
        claimed: Option<((u32, u32), u32)>,
    ) {
        let (g, encoder) = (ctx.g, ctx.encoder);
        if samples_cast {
            let [x, y] = grid.samples;
            let pass = g.compute_pass(encoder, ctx.stamp(GpuSpanName::raster_box).pass());
            g.set_bind_group(pass, 0, self.groups[0], &[]);
            g.set_pipeline(pass, self.sample_hit.id);
            g.dispatch(pass, x.div_ceil(SQUARE_SIDE), y.div_ceil(SQUARE_SIDE), 1);
            g.end(pass);
        }
        if draws_record {
            self.record_draw(ctx);
        }
        // The depth is reversed, nearer greater (pxl-engine `raster::NEAR`). Gated, it is
        // cleared to the nearest, which no box passes, and the gate sets the claimed tiles back
        // to the far plane; otherwise it is cleared to the far plane.
        let pass = g.pass(
            encoder,
            &pass_desc(
                self.visibility_view,
                Load::Clear,
                self.depth_view,
                Load::Clear,
                if claimed.is_some() { 1.0 } else { 0.0 },
                ctx.stamp(GpuSpanName::sraster).pass(),
            ),
        );
        g.declare_pass(pass, sequence::RASTER_PASS);
        if let Some((tiles, cnt)) = claimed {
            let group = g.bind_group(
                self.mask.layouts[0],
                &[buf_entry(30, tiles.0), buf_entry(31, tiles.1)],
            );
            g.set_pipeline(pass, self.mask.id);
            g.set_bind_group(pass, 0, group, &[]);
            g.draw(pass, 6, cnt, 0, 0);
            release(group);
        }
        g.set_pipeline(pass, self.draw_pipeline.id);
        g.set_bind_group(pass, 0, self.groups[1], &[]);
        g.draw_indirect(pass, self.args, sequence::RASTER.at());
        g.end(pass);
        // What lies about each new face the samples opened is asked before the pixels read it
        // off their visible voxel, and only where the samples were cast.
        if !samples_cast {
            return;
        }
        let pass = g.compute_pass(encoder, ctx.stamp(GpuSpanName::face).pass());
        g.set_bind_group(pass, 0, self.groups[2], &[]);
        g.set_pipeline(pass, self.face.id);
        g.dispatch_indirect(pass, self.args, sequence::FACE.at());
        g.end(pass);
    }

    /// Writes every drawable entry in view as a box and marks it drawn, after setting the
    /// draw's args to no box. Where the light marks the world's receivers this frame, each box
    /// also marks the tiles it stands on (record_draw.wgsl `record_draw_cs`, pxl-engine
    /// `light::HEADER_WORLD_RECEIVERS`). It runs once a frame at most: in the raster's draw, or,
    /// where the eye's pixels are not cast, in the light's cast ahead of its receivers' passes
    /// (ray/cast.rs).
    pub(super) fn record_draw(&self, ctx: &mut RenderContext) {
        let (g, encoder) = (ctx.g, ctx.encoder);
        g.write_buf(
            self.args,
            sequence::OPEN_RASTER_DRAW_ARGS.to_at(),
            bytes(&draw_args()),
        );
        let pass = g.compute_pass(encoder, ctx.stamp(GpuSpanName::raster_box).pass());
        g.set_bind_group(pass, 0, self.groups[3], &[]);
        g.set_pipeline(pass, self.record_draw_pipeline.id);
        g.dispatch(pass, node_cache::ENTRIES / record::DRAW_LANES, 1, 1);
        g.end(pass);
    }
}

/// The draw's args before the record's draw writes a box: a box's vertices, and no instance
/// (pxl-engine `raster::ARGS_WORDS`).
fn draw_args() -> [u32; raster::ARGS_WORDS as usize] {
    let mut args = [0; raster::ARGS_WORDS as usize];
    args[raster::ARGS_VERTICES as usize] = raster::VERTICES;
    args
}

impl Raster {
    /// Makes the light's buffers and its targets at the map's `size` where they are not made,
    /// and says whether they are.
    pub(super) fn light_made(&mut self, g: &Gpu, size: u32) -> bool {
        let s = &mut self.light;
        if s.uniform == 0 {
            let storage = buffer_usage::STORAGE | buffer_usage::COPY_DST;
            s.draws = g.buf(word_bytes(record::LIGHT_DRAWS as usize), storage);
            s.args = g.buf(
                word_bytes(raster::WORDS as usize),
                storage | buffer_usage::INDIRECT | buffer_usage::COPY_SRC,
            );
            s.hits = g.buf(
                word_bytes(3),
                buffer_usage::COPY_DST | buffer_usage::INDIRECT,
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
        }
        s.uniform != 0
    }

    /// Releases the light's buffers and targets as shadows go off. `light_made` makes them
    /// again.
    pub(super) fn light_release(&mut self) {
        let s = core::mem::take(&mut self.light);
        for id in [
            s.group,
            s.record_group,
            s.record_hits_group,
            s.visibility_view,
            s.visibility_array,
            s.visibility,
            s.depth_view,
            s.depth,
            s.uniform,
            s.args,
            s.hits,
            s.draws,
        ] {
            release(id);
        }
    }

    /// Writes the light's view off the eye's words `e` (pxl-engine `light::Light`) for a map
    /// `size` texels a side, laid as `prepare` lays the eye's, and clears the args. The light
    /// marks the entries it uses with the node cache's frame, as the eye does.
    pub(super) fn light_prepare(&self, g: &Gpu, e: &[u32], size: u32, starts: &light::StartGrid) {
        let s = &self.light;
        if s.uniform == 0 {
            return;
        }
        let words: &[u32; eye::LIGHT_WORDS] = e[eye::LIGHT..eye::LIGHT + eye::LIGHT_WORDS]
            .try_into()
            .unwrap_or(&[0; eye::LIGHT_WORDS]);
        let v = light::Light::of_words(words);
        // Where the ray of box texel (0, 0) starts. The light's boxes are laid in units from
        // there (light_aim.wgsl `texel_start`).
        let origin = starts.start(light::CORNER.map(|k| words[k] as i32));
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
            // No jitter. The origin is where box texel (0, 0)'s ray starts, which the light's
            // boxes are laid from (record_draw.wgsl `light_record_draw_cs`).
            0.0,
            0.0,
            0.0,
            0.0,
            f32::from_bits(origin[0] as u32),
            f32::from_bits(origin[1] as u32),
            f32::from_bits(origin[2] as u32),
            0.0,
        ];
        g.write_buf(s.uniform, 0, bytes(&uniform));
        let mut args = [0u32; raster::ARGS_WORDS as usize];
        args[raster::ARGS_VERTICES as usize] = raster::VERTICES;
        g.write_buf(
            s.args,
            sequence::OPEN_LIGHT_RASTER_ARGS.to_at(),
            bytes(&args),
        );
    }

    /// Makes the light's draw groups again where what they bind changed: the eye and the node
    /// cache the raster binds, and `light`, the light's head, list, args, stood list's args and
    /// map. Only the holes' stops' group binds the map (`LIGHT_RECORD_HITS_GROUP`).
    pub(super) fn light_record_group(&mut self, g: &Gpu, light: [u32; 5]) {
        let [eye, _, node_cache, ..] = self.bindings;
        let [header, list, args, receiver_tiles, map] = light;
        let bound = [eye, node_cache, header, list, args, receiver_tiles, map];
        let s = &mut self.light;
        if eye == 0 || s.uniform == 0 || bound == s.record_bufs {
            return;
        }
        release(s.group);
        s.group = g.bind_group(
            self.draw_pipeline.layouts[0],
            &[
                buf_entry(1, s.uniform),
                buf_entry(2, s.draws),
                buf_entry(9, node_cache),
            ],
        );
        let light_args = s.args;
        let buf = |b| match b {
            NODE_CACHE => node_cache,
            LIGHT_RASTER_ARGS => light_args,
            LIGHT_LIST => list,
            LIGHT_ARGS => args,
            LIGHT_RECEIVER_TILES => receiver_tiles,
            _ => 0,
        };
        release(s.record_group);
        let rest = [
            buf_entry(0, eye),
            buf_entry(14, s.draws),
            buf_entry(HEADER_BINDING, header),
        ];
        s.record_group = g.frame_group(
            self.light_record_draw.layouts[0],
            &LIGHT_RECORD_BINDS,
            buf,
            &rest,
        );
        release(s.record_hits_group);
        let rest = [
            buf_entry(0, eye),
            view_entry(3, map),
            buf_entry(HEADER_BINDING, header),
        ];
        s.record_hits_group = g.frame_group(
            self.light_record_hit.layouts[0],
            &LIGHT_RECORD_HITS_BINDS,
            buf,
            &rest,
        );
        s.record_bufs = bound;
    }

    /// Records the light's holes' stops after the light's queue, as `record_hits` does the eye's.
    /// Their dispatch is copied out of the node cache's head, then a pass timed in `span`
    /// writes them into their entries (record_hits.wgsl over record_hits.light.wgsl). The
    /// light's next draw takes them. The light's slots read no face word, so no face is asked
    /// or put.
    pub(super) fn light_record_hits(
        &self,
        ctx: &mut RenderContext,
        node_cache: u32,
        span: GpuSpanName,
    ) {
        let s = &self.light;
        if s.record_hits_group == 0 {
            return;
        }
        let (g, encoder) = (ctx.g, ctx.encoder);
        g.copy_buf(
            encoder,
            (node_cache, sequence::LIGHT_RECORD_HITS_DISPATCH.from_at()),
            (s.hits, sequence::LIGHT_RECORD_HITS_DISPATCH.to_at()),
            word_bytes(3),
        );
        let pass = g.compute_pass(encoder, ctx.stamp(span).pass());
        g.set_bind_group(pass, 0, s.record_hits_group, &[]);
        g.set_pipeline(pass, self.light_record_hit.id);
        g.dispatch_indirect(pass, s.hits, sequence::LIGHT_RECORD_HITS.at());
        g.end(pass);
    }

    /// Writes the drawable entries on the light's box as boxes and marks them drawn
    /// (record_draw.wgsl `light_record_draw_cs`), then draws them into the light's cleared
    /// targets.
    pub(super) fn light_draw(&self, ctx: &mut RenderContext) {
        let (g, encoder) = (ctx.g, ctx.encoder);
        let stamp = ctx.stamp(GpuSpanName::light_raster);
        let s = &self.light;
        let pass = g.compute_pass(encoder, stamp.pass());
        g.set_bind_group(pass, 0, s.record_group, &[]);
        g.set_pipeline(pass, self.light_record_draw.id);
        g.dispatch(pass, node_cache::ENTRIES / record::DRAW_LANES, 1, 1);
        g.end(pass);
        let stamp = ctx.stamp(GpuSpanName::light_raster);
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
        g.declare_pass(pass, sequence::LIGHT_RASTER_PASS);
        g.set_pipeline(pass, self.draw_pipeline.id);
        g.set_bind_group(pass, 0, s.group, &[]);
        g.draw_indirect(pass, s.args, sequence::LIGHT_RASTER.at());
        g.end(pass);
    }
}
