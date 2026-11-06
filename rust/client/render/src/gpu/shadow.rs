// pxl-render/gpu/shadow.rs

//! The light's shadow map: the world's depth from the light each frame,
//! orthographic about the eye and as wide as every effect reaches (the
//! kernel's `STAGE_AT::EFFECT_REACH`), so its texel follows the camera grid.
//! The world is cast a ray a texel through the world's query, as the eye's
//! rays are, off samples and the visible voxel (light.wgsl `light_cs`), into
//! `world`, a clipmap the light box slides over a texel at a time, every
//! texel each frame it is cast (pxl-engine `light`); laid into the map
//! each frame in the box's own order in the tiles a receiver reads, a quad each
//! (`PIPELINE_LIGHT_MAP`, `light::LISTED_READ`), the rest cleared to the far plane, and
//! decor and the actors, which move every frame, drawn over it depth only, from the light's
//! frame. The lit passes read
//! the map as a plain light box. At 2048 the world is 16 MiB and the map,
//! with its stencil, 20; both are made while the shadows are on alone
//! (`Shadow::made`). Where only the listed casters that changed moved over
//! the world its cast left as it was, the map is drawn again in the tiles
//! they set alone (gpu/tiles.rs), through a gate of its own
//! (light_gate.wgsl), as the world's pass is.

use crate::gpu::ray::sequence;
use pxl_engine::beam::{CORNER_SIDE, SAMPLE_SIDE};
use pxl_engine::eye;
use pxl_engine::light::{self, Light, Starts, ANCHOR};
use pxl_engine::param::light::SAMPLE;
use pxl_engine::ray::{MISS_ACROSS, MISS_LANES};
use pxl_engine::WORKGROUPS_MOST;
use pxl_spec::space::coord::mat4::{self, Mat4};
use pxl_spec::space::coord::v3::{self, V3};
use xpute_gpu::enums::{AddressMode, CompareFunction, FilterMode, MipmapFilterMode};
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::frame::shader::record::{PIPELINE_DECOR_DEPTH, PIPELINE_LIGHT_GATE, PIPELINE_LIGHT_MAP};
use crate::frame::shader::word::frame::{EYE, VIEW_PROJ};
use crate::frame::shader::word::FRAME_FLOATS;
use crate::gpu::api::{
    buf_entry, buffer_usage, bytes, pass_desc, release, sampler_entry, texture_usage, view_entry,
    word_bytes, Entry, Format, Gpu, Group, Load, RenderContext, Stencil, TrackedRenderPass,
    GATED_DEPTH,
};
use crate::gpu::pipeline::{Pipeline, Pipelines};
use crate::gpu::put::put_mat;
use crate::gpu::ray::LightReads;
use crate::gpu::span_spec::GpuSpanName;
use crate::gpu::tiles::Tiles;
use crate::param::shadow::DEPTH_STEP;

/// The map's side in texels.
pub const SIZE: u32 = 2048;
const _: () = assert!(
    SIZE as i32 <= ANCHOR,
    "the box lies in the four anchors about its corner (`Shadow::starts`)"
);
const _: () = {
    let most = WORKGROUPS_MOST;
    let per = light::LIST_ACROSS * light::LISTED_LANES;
    let corners = light::beam_across(SIZE) * light::beam_across(SIZE);
    assert!(
        (SIZE * SIZE).div_ceil(per) <= most
            && corners.div_ceil(per) <= most
            && (SIZE * SIZE).div_ceil(MISS_LANES * MISS_ACROSS) <= most
            && corners.div_ceil(CORNER_SIDE * CORNER_SIDE) <= most
            && ((SIZE / SAMPLE) * (SIZE / SAMPLE)).div_ceil(SAMPLE_SIDE * SAMPLE_SIDE) <= most,
        "each pass over what the light's cast lists fits one dispatch: its texels' and beam rays' lists, the misses and what the walks over the node cache left"
    );
};

/// An anchor of the rays' starts (`Shadow::starts`): its world texel, its
/// start in Q3 and what rounding to it left, in fixed point (`light::fixed`).
type Anchor = ([i32; 2], [i32; 3], [i32; 3]);

/// WebGPU clip z in [0, 1], column-major.
fn ortho(out: &mut Mat4, half: f64, near: f64, far: f64) {
    *out = [0.0; 16];
    out[0] = 1.0 / half;
    out[5] = 1.0 / half;
    out[10] = -1.0 / (far - near);
    out[14] = -near / (far - near);
    out[15] = 1.0;
}

/// What a cast map holds, made while the shadows are on (`Shadow::made`):
/// the map as its pass draws it, with the gate's stencil (`GATED_DEPTH`),
/// and its depth alone, as the lit passes read it; the world's depth from
/// the light as its cast leaves it, a word a slot of the clipmap (pxl-engine
/// `light::word`), which `light_cs` writes and `PIPELINE_LIGHT_MAP` lays
/// into the map off the box's corner.
struct Targets {
    map: u32,
    map_view: u32,
    map_depth_view: u32,
    world: u32,
    world_view: u32,
}

pub struct Shadow {
    /// The map and the world, none while the shadows are off.
    targets: Option<Targets>,
    sampler: u32,
    blank_view: u32,
    blank_sampler: u32,
    /// The world laid in, over `lay_group`: the world, the corner and the
    /// light's list whose reads hold the epoch's uncast tiles lit; and the
    /// map's gate, which lays it in the claimed tiles alone, over
    /// `gate_group`: those and the map's tiles' words and uniform and the
    /// marks' box (`Tiles::marks_box`). Both set before the frame
    /// (`set_groups`).
    lay: Pipeline,
    gate: Pipeline,
    lay_group: Group,
    gate_group: Group,
    corner: u32,
    /// Group 0 of the casters' depth pipelines: the light's frame.
    pub light_group: u32,
    light_frame: u32,
    light: [f32; FRAME_FLOATS],
    pub light_view_proj: Mat4,
    /// The world's cast's words off it (pxl-engine `eye::LIGHT`).
    pub light_words: [u32; eye::LIGHT_WORDS],
    /// The normal offset a fragment takes: a texel at the last reach.
    pub texel: f64,
    /// The light the map is cast from, moved only when the real one has gone a
    /// step from it.
    pub cast_from: Option<V3>,
    /// The light's frame the clipmap's texels are cast in, numbered: moved
    /// where the light the map is cast from, the reach or the depth step
    /// (`DEPTH_STEP`) moved, the rays' starts with it (`starts`).
    pub generation: u32,
    key: Option<(V3, f64, i64)>,
    /// What the rays' starts are worked out off (`starts`): the box's
    /// corner, the light's inverse and the eye's world position, as last
    /// aimed; and the anchors worked out in the light frame numbered first.
    box_corner: [i32; 2],
    inverse: Mat4,
    eye_world: V3,
    anchors: (u32, Vec<Anchor>),
}

impl Shadow {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines) -> Shadow {
        let sampler = g.sampler(
            FilterMode::linear,
            FilterMode::linear,
            MipmapFilterMode::nearest,
            AddressMode::clamp_to_edge,
            Some(CompareFunction::less_equal),
            1,
        );
        let blank = g.texture(
            1,
            1,
            1,
            Format::depth32float,
            1,
            texture_usage::TEXTURE_BINDING,
        );
        let blank_view = g.view(blank, false, 0, 1, 0, 1);
        let blank_sampler = g.sampler(
            FilterMode::nearest,
            FilterMode::nearest,
            MipmapFilterMode::nearest,
            AddressMode::clamp_to_edge,
            Some(CompareFunction::less_equal),
            1,
        );
        let depth = Format::depth32float;
        let lay = pipelines.build_gated(g, PIPELINE_LIGHT_MAP, depth, Stencil::Unused);
        let gate = pipelines.build_gated(g, PIPELINE_LIGHT_GATE, depth, Stencil::Gate);
        let corner = g.buf(
            word_bytes(8),
            buffer_usage::UNIFORM | buffer_usage::COPY_DST,
        );
        let light_frame = g.buf(
            word_bytes(FRAME_FLOATS),
            buffer_usage::UNIFORM | buffer_usage::COPY_DST,
        );
        let light_group = g.bind_group(
            pipelines.build(g, PIPELINE_DECOR_DEPTH, g.canvas()).layouts[0],
            &[buf_entry(0, light_frame)],
        );
        Shadow {
            targets: None,
            sampler,
            blank_view,
            blank_sampler,
            lay,
            gate,
            lay_group: Group::default(),
            gate_group: Group::default(),
            corner,
            light_group,
            light_frame,
            light: [0.0; FRAME_FLOATS],
            light_view_proj: mat4::new(),
            light_words: [0; eye::LIGHT_WORDS],
            texel: 0.0,
            cast_from: None,
            generation: 0,
            key: None,
            box_corner: [0; 2],
            inverse: mat4::new(),
            eye_world: [0.0; 3],
            anchors: (u32::MAX, Vec::new()),
        }
    }

    /// The map and the world made where `on`, the shadows' setting
    /// (`STAGE_AT::SHADOWS`), and let go where not: the first map cast after
    /// they are made casts whole, as the light's cast does (ray/cast.rs
    /// `light_made`). Whether they
    /// were made or let go, so the lit passes' groups are made again over
    /// the map or the blank one (`entries`).
    pub fn made(&mut self, g: &Gpu, on: bool) -> bool {
        if on == self.targets.is_some() {
            return false;
        }
        match self.targets.take() {
            Some(t) => {
                for id in [t.world_view, t.world, t.map_depth_view, t.map_view, t.map] {
                    release(id);
                }
            }
            None => {
                let map = g.texture(
                    SIZE,
                    SIZE,
                    1,
                    GATED_DEPTH,
                    1,
                    texture_usage::RENDER_ATTACHMENT | texture_usage::TEXTURE_BINDING,
                );
                let world = g.texture(
                    SIZE,
                    SIZE,
                    1,
                    Format::r32uint,
                    1,
                    texture_usage::STORAGE_BINDING | texture_usage::TEXTURE_BINDING,
                );
                let world_view = g.view(world, true, 0, 1, 0, 1);
                self.targets = Some(Targets {
                    map,
                    map_view: g.view(map, false, 0, 1, 0, 1),
                    map_depth_view: g.depth_only_view(map),
                    world,
                    world_view,
                });
            }
        }
        true
    }

    /// The lay's group and the gate's, over the map's `tiles` and the
    /// light's list `reads` (ray/cast.rs `light_reads`), before the frame's
    /// commands, where the map and the list are made; let go where either
    /// is not.
    pub fn set_groups(&mut self, g: &Gpu, tiles: &Tiles, reads: Option<u32>) {
        let (Some(t), Some(reads)) = (&self.targets, reads) else {
            self.lay_group.release();
            self.gate_group.release();
            return;
        };
        let lay = [
            view_entry(0, t.world_view),
            buf_entry(1, self.corner),
            buf_entry(2, reads),
        ];
        self.lay_group.set(g, self.lay.layouts[0], &lay);
        let (words, uniform) = tiles.this_frame();
        self.gate_group.set(
            g,
            self.gate.layouts[0],
            &[
                lay[0],
                lay[1],
                lay[2],
                buf_entry(30, words),
                buf_entry(31, uniform),
                buf_entry(33, tiles.marks_box()),
            ],
        );
    }

    /// Whether the map and the world are made (`made`).
    pub fn on(&self) -> bool {
        self.targets.is_some()
    }

    /// The world's depth from the light, as the light's cast writes it; 0
    /// while the shadows are off.
    pub fn world(&self) -> u32 {
        self.targets.as_ref().map_or(0, |t| t.world_view)
    }

    fn targets(&self) -> &Targets {
        self.targets.as_ref().or_bug(Errno::ENOTRECOVERABLE)
    }

    /// The light's frame, which a draw from the light reads in group 0, and
    /// the map as its pass draws it: what a mark from the light binds and
    /// attaches (gpu/pass/skinned_mesh.rs `draw_marks`).
    pub fn frame_and_map(&self) -> (u32, u32) {
        (self.light_frame, self.targets().map_view)
    }

    /// The map and its sampler, at `binding` and the next, as a lit pass
    /// reads them (shadow.wgsl); the blank one while the shadows are off.
    pub fn entries(&self, binding: u32) -> [Entry; 2] {
        match &self.targets {
            Some(t) => [
                view_entry(binding, t.map_depth_view),
                sampler_entry(binding + 1, self.sampler),
            ],
            None => self.blank_entries(binding),
        }
    }

    /// A map no pass draws, for a pass that draws none: every face reads it unshadowed.
    pub fn blank_entries(&self, binding: u32) -> [Entry; 2] {
        [
            view_entry(binding, self.blank_view),
            sampler_entry(binding + 1, self.blank_sampler),
        ]
    }

    /// `half_width` is the map's half width about `target`,
    /// `reach_ndc` the share of it a receiver reads within; `fixed`
    /// is a point that stays put in the world as the eye moves, in the eye's
    /// frame.
    pub fn aim(&mut self, sun_dir: &[f32], target: V3, half_width: f64, reach_ndc: f32, fixed: V3) {
        self.texel = half_width * 2.0 / f64::from(SIZE);
        // A step turns the map's edge by a texel, so a shadow ticks a texel on as
        // the light goes; turned on every frame, the grid below snaps another
        // way each time and a wall's shadow flickers.
        let light = v3::norm(&[sun_dir[0] as f64, sun_dir[1] as f64, sun_dir[2] as f64]);
        let step = self.texel / half_width;
        let from = match self.cast_from {
            Some(from) if v3::dot(&from, &light) > libm::cos(step) => from,
            _ => light,
        };
        self.cast_from = Some(from);
        // Two reaches up the light, so the sphere of `half_width` about the
        // target lies between the planes a reach either side of it, whatever
        // the light's angle, and a share past them each side (`DEPTH_STEP`).
        let eye: V3 = core::array::from_fn(|k| target[k] + from[k] * 2.0 * half_width);
        let (mut view, mut proj) = (mat4::new(), mat4::new());
        mat4::look_at(&mut view, eye, target, [0.0, 0.0, 1.0]);
        let margin = half_width * DEPTH_STEP / (1.0 - DEPTH_STEP);
        ortho(
            &mut proj,
            half_width,
            half_width - margin,
            3.0 * half_width + margin,
        );
        mat4::mul(&mut self.light_view_proj, &proj, &view);
        // The map follows the eye, so its texels are slid onto a grid fixed in the
        // world (stable shadow maps, texel snapping); and its
        // planes onto a coarser one along the light, so a texel kept is the
        // same ray. Where the fixed point falls on the texels' grid is the
        // world's texel index of the box's first texel, X across and Y down
        // the rows: the clipmap's corner, a whole number of tiles
        // (`light::TILE`), so a tile of the box holds whole tiles of the world
        // and what the cast read of it moves with the box a tile at a time
        // (light_slide.wgsl).
        let m = &mut self.light_view_proj;
        let mut cell = [0i64; 3];
        let tile = f64::from(light::TILE);
        for (k, step, whole) in [(0, step, tile), (1, step, tile), (2, DEPTH_STEP, 1.0)] {
            let at = m[k] * fixed[0] + m[4 + k] * fixed[1] + m[8 + k] * fixed[2] + m[12 + k];
            let n = (at / (step * whole)).round_ties_even() * whole;
            m[12 + k] -= at - n * step;
            cell[k] = n as i64;
        }
        let corner = [cell[0].wrapping_neg() as i32, cell[1] as i32];
        let key = Some((from, half_width, cell[2]));
        if key != self.key {
            (self.generation, self.key) = (self.generation.wrapping_add(1), key);
        }
        put_mat(&mut self.light, VIEW_PROJ, &self.light_view_proj);
        // The world's cast reads the box off the eye's words: the light's
        // view and projection undone, the map's NDC to the eye's frame.
        let mut inverse = mat4::new();
        mat4::invert(&mut inverse, &self.light_view_proj);
        let column = |k: usize| {
            [
                inverse[4 * k] as f32,
                inverse[4 * k + 1] as f32,
                inverse[4 * k + 2] as f32,
            ]
        };
        self.light_words = Light::of(
            [column(0), column(1), column(2), column(3)],
            SIZE,
            corner,
            reach_ndc,
        )
        .words();
        (self.box_corner, self.inverse) = (corner, inverse);
        self.eye_world = fixed.map(|v| -v);
    }

    /// Where the world's cast starts each texel's ray (pxl-engine
    /// `light::Starts`): the anchors about the box's corner, each worked out
    /// once a light frame, on the near plane where its world texel's middle
    /// lies, in Q3 off the eye's world position, and kept while the frame
    /// holds (`anchors`), so a texel's start is the same in every frame
    /// cast in it; and a texel's steps, in Q3 steps.
    pub fn starts(&mut self) -> Starts {
        if self.anchors.0 != self.generation {
            self.anchors = (self.generation, Vec::new());
        }
        let corner = self.box_corner;
        let base = [
            corner[0].div_euclid(ANCHOR) * ANCHOR,
            corner[1].div_euclid(ANCHOR) * ANCHOR,
        ];
        let q3 = f64::from(1u32 << pxl_engine::FRAC_W);
        let texel = 2.0 / f64::from(SIZE);
        let (inverse, eye) = (self.inverse, self.eye_world);
        let mut anchor = |k: usize| {
            let w = [
                base[0].wrapping_add((k % 2) as i32 * ANCHOR),
                base[1].wrapping_add((k / 2) as i32 * ANCHOR),
            ];
            if let Some(&(_, at, frac)) = self.anchors.1.iter().find(|a| a.0 == w) {
                return (at, frac);
            }
            // Its middle's NDC on the near plane, as `Light::of` lays the box's
            // texels, then the light's inverse, then the world.
            let (i, j) = (
                f64::from(w[0].wrapping_sub(corner[0])),
                f64::from(w[1].wrapping_sub(corner[1])),
            );
            let (x, y) = (
                -1.0 + texel / 2.0 + i * texel,
                1.0 - texel / 2.0 - j * texel,
            );
            let p = |a: usize| {
                let rel = inverse[a] * x + inverse[4 + a] * y + inverse[12 + a];
                (eye[a] + rel) * q3
            };
            let step = f64::from(1u32 << light::STEP_BITS);
            let (at, frac): (Vec<i32>, Vec<i32>) = (0..3)
                .map(|a| {
                    let v = p(a);
                    let r = v.round_ties_even();
                    ((r as i64) as i32, ((v - r) * step).round_ties_even() as i32)
                })
                .unzip();
            let at = [at[0], at[1], at[2]];
            let frac = [frac[0], frac[1], frac[2]];
            self.anchors.1.push((w, at, frac));
            (at, frac)
        };
        let [a0, a1, a2, a3] = [0, 1, 2, 3].map(&mut anchor);
        // Only the anchors about the box are kept.
        self.anchors.1.retain(|a| {
            let d = [a.0[0].wrapping_sub(base[0]), a.0[1].wrapping_sub(base[1])];
            (0..=ANCHOR).contains(&d[0]) && (0..=ANCHOR).contains(&d[1])
        });
        let light = Light::of_words(&self.light_words);
        Starts {
            base,
            across: light.across.map(|v| light::fixed(v * q3 as f32)),
            down: light.down.map(|v| light::fixed(v * q3 as f32)),
            at: [a0.0, a1.0, a2.0, a3.0],
            frac: [a0.1, a1.1, a2.1, a3.1],
        }
    }

    /// The light's frame, where the casters' depth pipelines read their
    /// view.
    /// The eye's Q3 in the light's frame, as in the eye's: a draw from the
    /// light takes its position against the same eye (frame.wgsl `from_eye`).
    pub fn put_eye(&mut self, eye: [f32; 3]) {
        self.light[EYE..EYE + 3].copy_from_slice(&eye);
    }

    pub fn write(&self, g: &Gpu) {
        g.write_buf(self.light_frame, 0, bytes(&self.light));
    }

    /// The map's pass, timed in the span `shadow`, cleared to the far plane, the world's
    /// depth laid in first where `cast` (its cast holds this frame's world) in the tiles the
    /// light's cast last listed read, a quad each, as many as it opened (`light::ARGS_LAY`):
    /// every texel a receiver reads lies in one, and the rest hold the far plane, which none
    /// reads. In the claimed tiles of `tiles` alone where it is drawn through
    /// its gate (`gated`, its stencil numbered by `Tiles::gate`), a frame
    /// that casts, the rest kept as the last frame left it: the gate lays
    /// the world there and raises their stencil, a quad a tile of the box
    /// the frame's marks cover, the unclaimed collapsed, as many as the GPU
    /// counted (`Tiles::box_args`), each over its group set before the frame
    /// (`set_groups`). The casters are drawn into it next, through the
    /// stencil, then it ends.
    pub fn begin<'a>(
        &self,
        ctx: &mut RenderContext<'a>,
        cast: bool,
        tiles: &mut Tiles,
        gated: bool,
        LightReads { epoch, args, .. }: LightReads,
    ) -> TrackedRenderPass<'a> {
        let (g, encoder) = (ctx.g, ctx.encoder);
        let t = self.targets();
        // The box's corner the world's depth is laid in off, and the reads that tell the
        // tiles the light's epoch cast from those it has not yet, past where a slide went
        // unrun; and the tiles read (light_lay.wgsl `Lay`).
        let corner = [
            self.light_words[light::CORNER[0]],
            self.light_words[light::CORNER[1]],
            epoch,
            light::reads_at(SIZE),
            light::listed_at(SIZE, light::LISTED_READ),
            light::slide_at(SIZE) + light::READS_FROM,
            0,
            0,
        ];
        g.write_buf(self.corner, 0, bytes(&corner));
        let stamp = ctx.stamp(GpuSpanName::shadow);
        let load = if gated { Load::Load } else { Load::Clear };
        let desc = pass_desc(0, Load::Clear, t.map_view, load, 1.0, stamp.pass());
        let (desc, number) = tiles.gate(desc, gated);
        let pass = g.pass(encoder, &desc);
        let mut pass = TrackedRenderPass::new(g, pass);
        if let Some(n) = number {
            xpute_kit::ensure!(cast, EINVAL, n);
            pass.set_stencil_reference(n);
            pass.set_pipeline(self.gate.id);
            pass.set_bind_group(0, self.gate_group.id(), &[]);
            if let Some(args) = tiles.box_args() {
                pass.draw_indirect(args, sequence::MAP_GATE.at());
            }
        } else if cast {
            pass.set_pipeline(self.lay.id);
            pass.set_bind_group(0, self.lay_group.id(), &[]);
            pass.draw_indirect(args, sequence::MAP_LAY.at());
        }
        pass
    }
}
