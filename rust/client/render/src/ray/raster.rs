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
//! a gate of their own clears the depth to the nearest and sets the damaged tiles back to the
//! far plane (raster_mask.wgsl), so boxes pass the early depth test in those tiles alone; the
//! pixels' cast skips the other tiles and reads nothing there. The args and the boxes are
//! written again each frame the samples are cast, and kept where only the pixels are cast
//! again over them.

use crate::frame::shader::access::Group;
use crate::frame::shader::sequence;
use pxl_engine::{beam, eye, light, node_cache, raster, record};

use crate::frame::shader::group::{
    BEAMS, FACES_BINDS, HIT_FACES_OF_BINDS, LIGHT_ARGS, LIGHT_LIST, LIGHT_RASTER_ARGS,
    LIGHT_RECEIVER_TILES, LIGHT_REC_BINDS, LIGHT_REC_HITS_BINDS, NODE_CACHE, RASTER_ARGS,
    REC_BINDS, REC_HITS_OF_BINDS, SAMPLE_HIT_BINDS,
};
use crate::frame::shader::record::{
    COMPUTE_FACE, COMPUTE_LIGHT_REC_DRAW, COMPUTE_LIGHT_REC_HITS, COMPUTE_LIGHT_SAMPLE_HIT,
    COMPUTE_REC_DRAW, COMPUTE_REC_FACES, COMPUTE_REC_HITS, COMPUTE_SAMPLE_HIT, LIGHT_HDR_BINDING,
    PIPELINE_RASTER, PIPELINE_RASTER_MASK,
};
use crate::frame::shader::word::{BOX_VERTEX_CNT, RASTER_FLOATS, SQUARE_SIDE};
use crate::gpu::api;
use crate::gpu::api::{
    buf_entry, buffer_usage, bytes, frame_entries, pass_desc, release, texture_usage, view_entry,
    word_bytes, Entry, Format, Gpu, Load, RenderContext,
};
use crate::gpu::pipeline::{Pipeline, PipelineCache};
use crate::gpu::span_spec::GpuSpanName;
use pxl_engine::param::raster::GROWTH;

/// The eye's raster. Its targets are at the rays' size.
pub(super) struct Raster {
    /// Sets each sample's stop on its node's drawable entry (sample_hit.wgsl).
    pub(super) sample_hit: Pipeline,
    /// The passes that write drawable entries as boxes, a thread a slot of the node cache: the
    /// eye's for the entries in view, the light's for those on its box (record_draw.wgsl).
    rec_draw_pipeline: Pipeline,
    light_rec_draw: Pipeline,
    /// Writes the holes' stops into their drawable entries (record_hits.wgsl).
    rec_hit: Pipeline,
    /// Puts what lies about each hole's face at its pixel, read off its entry (record_hits.wgsl
    /// `rec_faces_cs`).
    face_put: Pipeline,
    /// Writes the light's holes' stops into their drawable entries (record_hits.wgsl over
    /// record_hits.light.wgsl).
    light_rec_hit: Pipeline,
    /// Sets the shadow map's samples' stops, as `sample_hit` sets the eye's (sample_hit.wgsl over
    /// sample.light.wgsl). Its group binds the light's head as well.
    pub(super) light_sample_hit: Pipeline,
    face: Pipeline,
    draw_pipeline: Pipeline,
    /// Draws the gate the boxes pass through in a frame not drawn whole: it sets the damaged
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
    pub(super) targets: Targets,
    size: [u32; 2],
    /// The groups, by index: the samples' stops, the boxes' draw, the samples' faces' asks, the
    /// record's draw (made by `rec_group`), the holes' faces' asks, and the holes' stops.
    groups: [api::Group; 6],
    /// The light's raster, which exists while shadows are on (`light_made`, `light_release`).
    pub(super) light: LightRaster,
}

/// The light's raster over the same drawable entries (pxl-engine `record`). The light's
/// samples and holes set their stops on the entries as the eye's do. Its draw writes a box for
/// each entry on the light's box (record_draw.wgsl `light_rec_draw_cs`) and draws them from
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
    pub(super) targets: Targets,
    group: api::Group,
    rec_group: api::Group,
    /// The group of its holes' stops (`LIGHT_REC_HITS_LAYOUT`), set with the record's draw
    /// group.
    rec_hits_group: api::Group,
}

/// A raster's targets: the visibility buffer, which holds each pixel's voxel (r32uint), and the
/// depth it is tested by. The visibility buffer is drawn into as a 2D view and read as an array
/// of one layer (`visibility_array`).
#[derive(Default)]
pub(super) struct Targets {
    visibility: u32,
    visibility_view: u32,
    pub(super) visibility_array: u32,
    depth: u32,
    depth_view: u32,
}

impl Targets {
    fn new(g: &Gpu, [w, h]: [u32; 2]) -> Targets {
        let usage = texture_usage::RENDER_ATTACHMENT | texture_usage::TEXTURE_BINDING;
        let visibility = g.texture(w, h, 1, Format::R32uint, 1, usage);
        let depth = g.texture(w, h, 1, Format::Depth32float, 1, usage);
        Targets {
            visibility,
            visibility_view: g.view(visibility, false, 0, 1, 0, 1),
            visibility_array: g.view(visibility, true, 0, 1, 0, 1),
            depth,
            depth_view: g.view(depth, false, 0, 1, 0, 1),
        }
    }

    fn release(&self) {
        for id in [
            self.visibility_view,
            self.visibility_array,
            self.visibility,
            self.depth_view,
            self.depth,
        ] {
            release(id);
        }
    }
}

/// A raster's view, as raster.wgsl's `Raster` lays it out, a vec4 a member: the view's axes,
/// each with its `w`, the turn and whether the view is the light's, the screen, the first ray's
/// offset and the origin the boxes are laid from.
struct View {
    right: [f32; 4],
    up: [f32; 4],
    forward: [f32; 4],
    turn: u32,
    light: bool,
    screen: [f32; 4],
    first: [f32; 2],
    origin: [i32; 3],
}

impl View {
    fn write(&self, g: &Gpu, uniform: u32) {
        let bits = |v: [f32; 4]| v.map(f32::to_bits);
        let [x, y] = self.first.map(f32::to_bits);
        let [ox, oy, oz] = self.origin.map(|v| v as u32);
        let words: [[u32; 4]; RASTER_FLOATS / 4] = [
            bits(self.right),
            bits(self.up),
            bits(self.forward),
            [self.turn, u32::from(self.light), 0, 0],
            bits(self.screen),
            [x, y, 0, 0],
            [ox, oy, oz, 0],
        ];
        g.write_buf(uniform, 0, bytes(words.as_flattened()));
    }
}

impl Raster {
    pub(super) fn new(g: &Gpu, pipelines: &mut PipelineCache) -> Raster {
        Raster {
            sample_hit: pipelines.build_compute(g, COMPUTE_SAMPLE_HIT),
            light_sample_hit: pipelines.build_compute(g, COMPUTE_LIGHT_SAMPLE_HIT),
            rec_draw_pipeline: pipelines.build_compute(g, COMPUTE_REC_DRAW),
            light_rec_draw: pipelines.build_compute(g, COMPUTE_LIGHT_REC_DRAW),
            rec_hit: pipelines.build_compute(g, COMPUTE_REC_HITS),
            face_put: pipelines.build_compute(g, COMPUTE_REC_FACES),
            light_rec_hit: pipelines.build_compute(g, COMPUTE_LIGHT_REC_HITS),
            face: pipelines.build_compute(g, COMPUTE_FACE),
            draw_pipeline: pipelines.build(g, PIPELINE_RASTER, Format::R32uint),
            mask: pipelines.build(g, PIPELINE_RASTER_MASK, Format::R32uint),
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
                word_bytes(record::EYE_DRAW_CAP as usize),
                buffer_usage::STORAGE | buffer_usage::COPY_DST,
            ),
            uniform: g.buf(
                word_bytes(RASTER_FLOATS),
                buffer_usage::UNIFORM | buffer_usage::COPY_DST,
            ),
            targets: Targets::default(),
            size: [0; 2],
            groups: Default::default(),
            light: LightRaster::default(),
        }
    }

    /// Readies the raster for a frame that casts the pixels, before its dispatches. It makes
    /// the targets again where the rays' `size` changed, and writes the raster's view off the
    /// eye's words `e`. Where the samples are cast again (`samples_cast`), it opens their faces'
    /// asks; otherwise the boxes the last cast left are kept. The groups bind the cast's eye, beams
    /// and node cache, the world query's buffers the faces' asks read, and the rays, at whose
    /// pixels the holes' faces are put (`rec_hits`).
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
            self.targets.release();
            self.targets = Targets::new(g, size);
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
        View {
            right: [r[0], r[1], r[2], tan[0]],
            up: [u[0], u[1], u[2], tan[1]],
            forward: [f[0], f[1], f[2], raster::NEAR],
            turn: BOX_VERTEX_CNT * u32::from(raster::is_flipped(r, u, f)),
            light: false,
            screen: [GROWTH * px, w, h, far],
            first: [e[eye::JITTER], e[eye::JITTER + 1]].map(f32::from_bits),
            origin: [0, 1, 2].map(|k| e[eye::ORIGIN + k] as i32),
        }
        .write(g, self.uniform);
        if samples_cast {
            // No face is asked yet. The faces' dispatch lies at `raster::ASKED_AT`, and its
            // asks start where the grid lays them (`beam::Grid::asks_at`). No box is drawn
            // yet either: the draw's args are opened in every frame the record's draw runs,
            // and it runs in every frame the samples are cast (ray/cast.rs `draws_rec`,
            // `open_draw`).
            g.write_buf(
                self.args,
                sequence::OPEN_RASTER_ASKS.to_at(),
                bytes(&faces_args(grid.asks_at)),
            );
        }
        // The holes' stops have opened no face ask yet. They run every frame the pixels are
        // cast, so this is written in every one; their faces' dispatch lies at
        // `raster::HIT_ASKED_AT`.
        g.write_buf(
            self.args,
            sequence::OPEN_RASTER_HIT_ASKS.to_at(),
            bytes(&faces_args(grid.hit_asks_at)),
        );
        let buf = |b| match b {
            BEAMS => beams,
            NODE_CACHE => node_cache,
            RASTER_ARGS => self.args,
            _ => 0,
        };
        let framed = |p: &Pipeline, table: &Group, rest: &[Entry]| {
            let layout = p.layouts[0];
            (layout, frame_entries(layout, table, buf, rest))
        };
        // One faces' pass serves the samples' stops and the holes', each through a group
        // bound to its own asks.
        let world = [
            buf_entry(0, eye),
            buf_entry(1, clipmap),
            buf_entry(2, ground),
            buf_entry(5, shards),
            buf_entry(6, objects),
            buf_entry(7, pool),
        ];
        let draw = [
            buf_entry(1, self.uniform),
            buf_entry(2, self.draws),
            buf_entry(9, node_cache),
        ];
        let made = [
            (
                0,
                framed(&self.sample_hit, &SAMPLE_HIT_BINDS, &[buf_entry(0, eye)]),
            ),
            (1, (self.draw_pipeline.layouts[0], draw.to_vec())),
            (2, framed(&self.face, &FACES_BINDS, &world)),
            (4, framed(&self.face, &HIT_FACES_OF_BINDS, &world)),
            (
                5,
                framed(
                    &self.rec_hit,
                    &REC_HITS_OF_BINDS,
                    &[buf_entry(0, eye), view_entry(3, rays)],
                ),
            ),
        ];
        // The record's draw group binds the light's buffers too, so `rec_group` sets it.
        for (k, (layout, e)) in made {
            self.groups[k].set(g, layout, &e);
        }
    }

    /// Sets the record's draw group (`REC_LAYOUT`) over the cast's `eye` and `node_cache`, and
    /// `light`, the light's head, list, args and stood list's args (light_receiver_tile.wgsl),
    /// by which the draw marks the tiles its boxes stand on. It is set apart from `prepare`, in
    /// every frame the cast prepares, as it binds the light's buffers too.
    pub(super) fn rec_group(&mut self, g: &Gpu, [eye, node_cache]: [u32; 2], light: [u32; 4]) {
        let [hdr, list, args, receiver_tiles] = light;
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
            buf_entry(LIGHT_HDR_BINDING, hdr),
        ];
        let layout = self.rec_draw_pipeline.layouts[0];
        self.groups[3].set(g, layout, &frame_entries(layout, &REC_BINDS, buf, &rest));
    }

    /// Records the holes' stops once the holes' queue is done (ray/cast.rs). Their dispatch is
    /// copied out of the node cache's head first (`hits`). Then, in one pass timed in `span`,
    /// the stops are written into their entries, the faces they opened are asked, and each
    /// stop's face word is put at its pixel off its entry.
    pub(super) fn rec_hits(&self, ctx: &mut RenderContext, node_cache: u32, span: GpuSpanName) {
        let (g, encoder) = (ctx.g, ctx.encoder);
        g.copy_buf(
            encoder,
            (node_cache, sequence::REC_HITS_DISPATCH.from_at()),
            (self.hits, sequence::REC_HITS_DISPATCH.to_at()),
            word_bytes(3),
        );
        let pass = g.compute_pass(encoder, ctx.stamp(span).pass());
        g.set_pipeline(pass, self.rec_hit.id);
        g.set_bind_group(pass, 0, self.groups[5].id(), &[]);
        g.dispatch_indirect(pass, self.hits, sequence::REC_HITS.at());
        g.set_pipeline(pass, self.face.id);
        g.set_bind_group(pass, 0, self.groups[4].id(), &[]);
        g.dispatch_indirect(pass, self.args, sequence::HIT_FACES.at());
        g.set_pipeline(pass, self.face_put.id);
        g.set_bind_group(pass, 0, self.groups[5].id(), &[]);
        g.dispatch_indirect(pass, self.hits, sequence::HIT_FACES_PUT.at());
        g.end(pass);
    }

    /// Draws the raster. Where the samples were cast again (`samples_cast`), their stops are set
    /// on the entries first. Where `draws_rec`, the entries in view are written as boxes
    /// again; otherwise the boxes the last draw wrote are kept. The boxes are then drawn into
    /// cleared targets, through the gate in the tiles of `damage` where the frame is not drawn
    /// whole.
    pub(super) fn draw(
        &self,
        ctx: &mut RenderContext,
        grid: &beam::Grid,
        (samples_cast, draws_rec): (bool, bool),
        damage: Option<((u32, u32), u32)>,
    ) {
        let (g, encoder) = (ctx.g, ctx.encoder);
        if samples_cast {
            let [x, y] = grid.sample_lattice;
            let pass = g.compute_pass(encoder, ctx.stamp(GpuSpanName::RasterBox).pass());
            g.set_bind_group(pass, 0, self.groups[0].id(), &[]);
            g.set_pipeline(pass, self.sample_hit.id);
            g.dispatch(pass, x.div_ceil(SQUARE_SIDE), y.div_ceil(SQUARE_SIDE), 1);
            g.end(pass);
        }
        if draws_rec {
            self.rec_draw(ctx);
        }
        // The depth is reversed, nearer greater (pxl-engine `raster::NEAR`). Gated, it is
        // cleared to the nearest, which no box passes, and the gate sets the damaged tiles back
        // to the far plane; otherwise it is cleared to the far plane.
        let pass = g.pass(
            encoder,
            &pass_desc(
                self.targets.visibility_view,
                Load::Clear,
                self.targets.depth_view,
                Load::Clear,
                if damage.is_some() { 1.0 } else { 0.0 },
                ctx.stamp(GpuSpanName::Sraster).pass(),
            ),
        );
        g.declare_pass(pass, sequence::RASTER_PASS);
        if let Some((tiles, cnt)) = damage {
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
        g.set_bind_group(pass, 0, self.groups[1].id(), &[]);
        g.draw_indirect(pass, self.args, sequence::RASTER.at());
        g.end(pass);
        // What lies about each new face the samples opened is asked before the pixels read it
        // off their visible voxel, and only where the samples were cast.
        if !samples_cast {
            return;
        }
        let pass = g.compute_pass(encoder, ctx.stamp(GpuSpanName::Face).pass());
        g.set_bind_group(pass, 0, self.groups[2].id(), &[]);
        g.set_pipeline(pass, self.face.id);
        g.dispatch_indirect(pass, self.args, sequence::FACE.at());
        g.end(pass);
    }

    /// Writes every drawable entry in view as a box and marks it drawn, into the draw's args
    /// `open_draw` opened. Where the light marks the world's receivers this frame, each box
    /// also marks the tiles it stands on (record_draw.wgsl `rec_draw_cs`, pxl-engine
    /// `light::HDR_WORLD_RECEIVERS`). It runs once a frame at most: in the raster's draw, or,
    /// where the eye's pixels are not cast, in the light's cast ahead of its receivers' passes
    /// (ray/cast.rs).
    pub(super) fn rec_draw(&self, ctx: &mut RenderContext) {
        let (g, encoder) = (ctx.g, ctx.encoder);
        let pass = g.compute_pass(encoder, ctx.stamp(GpuSpanName::RasterBox).pass());
        g.set_bind_group(pass, 0, self.groups[3].id(), &[]);
        g.set_pipeline(pass, self.rec_draw_pipeline.id);
        g.dispatch(pass, node_cache::ENTRY_CNT / record::DRAW_LANE_CNT, 1, 1);
        g.end(pass);
    }

    /// Opens the draw's args before the frame, with no box, in a frame whose record's draw
    /// runs (ray/cast.rs `draws_rec`), as `light_prepare` opens the light's.
    pub(super) fn open_draw(&self, g: &Gpu) {
        g.write_buf(
            self.args,
            sequence::OPEN_RASTER_DRAW_ARGS.to_at(),
            bytes(&draw_args()),
        );
    }
}

/// The draw's args before the record's draw writes a box: a box's vertices, and no instance
/// (pxl-engine `raster::ARGS_WORDS`).
fn draw_args() -> [u32; raster::ARGS_WORDS as usize] {
    let mut args = [0; raster::ARGS_WORDS as usize];
    args[raster::ARGS_VERTEX_CNT as usize] = raster::VERTEX_CNT;
    args
}

/// The faces' dispatch before any box opens a workgroup, its asks starting at word `asks_at`
/// of the beam rays' (pxl-engine `raster::ARGS_FACES`, `raster::ARGS_ASKS`).
fn faces_args(asks_at: u32) -> [u32; raster::ARGS_WORDS as usize] {
    let mut args = [0; raster::ARGS_WORDS as usize];
    args[raster::ARGS_FACES as usize + 1] = 1;
    args[raster::ARGS_FACES as usize + 2] = 1;
    args[raster::ARGS_ASKS as usize] = asks_at;
    args
}

impl Raster {
    /// Makes the light's buffers and its targets at the map's `size` where they are not made,
    /// and says whether they are.
    pub(super) fn light_made(&mut self, g: &Gpu, size: u32) -> bool {
        let s = &mut self.light;
        if s.uniform == 0 {
            let storage = buffer_usage::STORAGE | buffer_usage::COPY_DST;
            s.draws = g.buf(word_bytes(record::LIGHT_DRAW_CAP as usize), storage);
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
            s.targets = Targets::new(g, [size; 2]);
        }
        s.uniform != 0
    }

    /// Releases the light's buffers and targets as shadows go off. `light_made` makes them
    /// again.
    pub(super) fn light_release(&mut self) {
        let mut s = core::mem::take(&mut self.light);
        for group in [&mut s.group, &mut s.rec_group, &mut s.rec_hits_group] {
            group.release();
        }
        s.targets.release();
        for id in [s.uniform, s.args, s.hits, s.draws] {
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
        let n = size as f32;
        View {
            right: [r[0], r[1], r[2], length(v.across)],
            up: [d[0], d[1], d[2], length(v.down)],
            forward: [v.dir[0], v.dir[1], v.dir[2], 0.0],
            turn: BOX_VERTEX_CNT * u32::from(raster::is_flipped(r, up, v.dir)),
            light: true,
            screen: [GROWTH * length(v.across), n, n, v.t_max],
            // No jitter. The origin is where box texel (0, 0)'s ray starts, which the light's
            // boxes are laid from (record_draw.wgsl `light_rec_draw_cs`).
            first: [0.0; 2],
            origin,
        }
        .write(g, s.uniform);
        g.write_buf(
            s.args,
            sequence::OPEN_LIGHT_RASTER_ARGS.to_at(),
            bytes(&draw_args()),
        );
    }

    /// Sets the light's draw groups over the cast's `eye` and `node_cache`, and `light`, the
    /// light's head, list, args, stood list's args and map, while the light's raster exists.
    /// Only the holes' stops' group binds the map (`LIGHT_REC_HITS_LAYOUT`).
    pub(super) fn light_rec_group(
        &mut self,
        g: &Gpu,
        [eye, node_cache]: [u32; 2],
        light: [u32; 5],
    ) {
        let [hdr, list, args, receiver_tiles, map] = light;
        let s = &mut self.light;
        if s.uniform == 0 {
            return;
        }
        s.group.set(
            g,
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
        let rest = [
            buf_entry(0, eye),
            buf_entry(14, s.draws),
            buf_entry(LIGHT_HDR_BINDING, hdr),
        ];
        let layout = self.light_rec_draw.layouts[0];
        s.rec_group.set(
            g,
            layout,
            &frame_entries(layout, &LIGHT_REC_BINDS, buf, &rest),
        );
        let rest = [
            buf_entry(0, eye),
            view_entry(3, map),
            buf_entry(LIGHT_HDR_BINDING, hdr),
        ];
        let layout = self.light_rec_hit.layouts[0];
        s.rec_hits_group.set(
            g,
            layout,
            &frame_entries(layout, &LIGHT_REC_HITS_BINDS, buf, &rest),
        );
    }

    /// Records the light's holes' stops after the light's queue, as `rec_hits` does the eye's.
    /// Their dispatch is copied out of the node cache's head, then a pass timed in `span`
    /// writes them into their entries (record_hits.wgsl over record_hits.light.wgsl). The
    /// light's next draw takes them. The light's slots read no face word, so no face is asked
    /// or put.
    pub(super) fn light_rec_hits(
        &self,
        ctx: &mut RenderContext,
        node_cache: u32,
        span: GpuSpanName,
    ) {
        let s = &self.light;
        if s.rec_hits_group.id() == 0 {
            return;
        }
        let (g, encoder) = (ctx.g, ctx.encoder);
        g.copy_buf(
            encoder,
            (node_cache, sequence::LIGHT_REC_HITS_DISPATCH.from_at()),
            (s.hits, sequence::LIGHT_REC_HITS_DISPATCH.to_at()),
            word_bytes(3),
        );
        let pass = g.compute_pass(encoder, ctx.stamp(span).pass());
        g.set_bind_group(pass, 0, s.rec_hits_group.id(), &[]);
        g.set_pipeline(pass, self.light_rec_hit.id);
        g.dispatch_indirect(pass, s.hits, sequence::LIGHT_REC_HITS.at());
        g.end(pass);
    }

    /// Writes the drawable entries on the light's box as boxes and marks them drawn
    /// (record_draw.wgsl `light_rec_draw_cs`), then draws them into the light's cleared
    /// targets.
    pub(super) fn light_draw(&self, ctx: &mut RenderContext) {
        let (g, encoder) = (ctx.g, ctx.encoder);
        let stamp = ctx.stamp(GpuSpanName::LightRaster);
        let s = &self.light;
        let pass = g.compute_pass(encoder, stamp.pass());
        g.set_bind_group(pass, 0, s.rec_group.id(), &[]);
        g.set_pipeline(pass, self.light_rec_draw.id);
        g.dispatch(pass, node_cache::ENTRY_CNT / record::DRAW_LANE_CNT, 1, 1);
        g.end(pass);
        let stamp = ctx.stamp(GpuSpanName::LightRaster);
        let pass = g.pass(
            encoder,
            &pass_desc(
                s.targets.visibility_view,
                Load::Clear,
                s.targets.depth_view,
                Load::Clear,
                0.0,
                stamp.pass(),
            ),
        );
        g.declare_pass(pass, sequence::LIGHT_RASTER_PASS);
        g.set_pipeline(pass, self.draw_pipeline.id);
        g.set_bind_group(pass, 0, s.group.id(), &[]);
        g.draw_indirect(pass, s.args, sequence::LIGHT_RASTER.at());
        g.end(pass);
    }
}
