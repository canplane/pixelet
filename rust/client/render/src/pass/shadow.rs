// pxl-render/pass/shadow.rs

//! The light's shadow map: the world's depth seen from the light, orthographic, centered on
//! the eye and as wide as every effect reaches (`EFFECT_REACH`). Its texels are snapped to a
//! grid fixed in the world (`Shadow::aim`). The world is cast a ray a texel through the
//! world's query, as the eye's rays are (light.wgsl `light_cs`), into `world`, a clipmap over
//! which the light box slides a texel at a time (pxl-engine `light`). Each frame the world's
//! depth is laid into the map, in the box's own texel order, in the tiles a receiver reads, a
//! quad a tile (`PIPELINE_LIGHT_MAP`, `light::LIST_READS`); the rest of the map is cleared to
//! the far plane. Decor and the characters, which move every frame, are then drawn over it
//! from the light's frame, depth only. The lit passes read the map as a plain orthographic
//! shadow map. Where only the listed casters that changed moved, over a world the cast left
//! as it was, the map is drawn again only in the tiles they set (gpu/tile.rs), through a
//! gate of its own (light_mask.wgsl), as the world's pass is. At a side of 2048 the world
//! takes 16 MiB and the map with its stencil 20 MiB, and both exist only while the shadows
//! are on (`Shadow::made`).

use crate::frame::shader::sequence;
use pxl_engine::cast::REACH;
use pxl_engine::eye;
use pxl_engine::light::{self, Light, StartGrid, ANCHOR};
use pxl_engine::WORKGROUPS_CAP;
use pxl_spec::space::coord::mat4::{self, Mat4};
use pxl_spec::space::coord::q3::FRAC_W;
use pxl_spec::space::coord::v3::{self, V3};
use pxl_spec::space::layer::lattice::EFFECT_REACH;
use xpute_gpu_link::enums::{AddressMode, CompareFunction, FilterMode, MipmapFilterMode};
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::frame::shader::record::{PIPELINE_DECOR_DEPTH, PIPELINE_LIGHT_MAP, PIPELINE_LIGHT_MASK};
use crate::frame::shader::word::frame::{DRIFT_Q, EYE, VIEW_PROJ};
use crate::frame::shader::word::{FRAME_FLOATS, SHADOW_SIZE};
use crate::gpu::api::{
    buf_entry, buffer_usage, bytes, pass_desc, release, sampler_entry, texture_usage, view_entry,
    word_bytes, Entry, Format, Gpu, Group, Load, RenderContext, Stencil, TrackedRenderPass,
    MASKED_DEPTH,
};
use crate::gpu::pipeline::{Pipeline, PipelineCache};
use crate::gpu::put::put_mat;
use crate::gpu::span_spec::GpuSpanName;
use crate::gpu::tile::TileGrid;
use crate::param::shadow::DEPTH_STEP;
use crate::ray::LightRead;

/// The light box's half width about its target for the effects' `reach`: the least power of
/// two at or above `reach`, so the snap grid and the texel's size change only in steps.
pub const fn half_width(reach: f64) -> f64 {
    let mut h = 1.0;
    while h < reach {
        h *= 2.0;
    }
    while h / 2.0 >= reach && h > f64::MIN_POSITIVE {
        h /= 2.0;
    }
    h
}

/// How far from its ray's start any node the light's cast meets can lie, for a box of half
/// width `half_width` about its target (`aim`). Along the light, the distance is the box's
/// depth, which is the rays' reach (`Light::t_max`); across, it is the box's full width on
/// each of its two axes.
pub const fn light_reach(half_width: f64) -> f64 {
    2.0 * half_width / (1.0 - DEPTH_STEP) + 4.0 * half_width
}
const _: () = assert!(
    light_reach(half_width(EFFECT_REACH)) < REACH as f64,
    "every node the light's cast meets is one the node cache keeps and the record names again from its box (pxl-engine cast::REACH)"
);
const _: () = assert!(
    SHADOW_SIZE as i32 <= ANCHOR,
    "the box lies in the four anchors about its corner (`Shadow::starts`)"
);
const _: () = {
    let cap = WORKGROUPS_CAP;
    let per = light::LIST_ACROSS * light::LIST_LANES;
    let corners = light::beam_across(SHADOW_SIZE) * light::beam_across(SHADOW_SIZE);
    assert!(
        (SHADOW_SIZE * SHADOW_SIZE).div_ceil(per) <= cap && corners.div_ceil(per) <= cap,
        "each pass over what the light's cast lists fits one dispatch: its texels' and beam rays' lists"
    );
};

/// An anchor of the rays' starts (`Shadow::starts`): its world texel, its start in Q3, and the
/// remainder that rounding the start to Q3 left, in fixed point (`light::fixed`).
type Anchor = ([i32; 2], [i32; 3], [i32; 3]);

/// An orthographic projection of half width `half` between the planes `near` and `far`,
/// column-major, into WebGPU's clip z range of 0 to 1.
fn ortho(out: &mut Mat4, half: f64, near: f64, far: f64) {
    *out = [0.0; 16];
    out[0] = 1.0 / half;
    out[5] = 1.0 / half;
    out[10] = -1.0 / (far - near);
    out[14] = -near / (far - near);
    out[15] = 1.0;
}

/// The textures the shadows hold while they are on (`Shadow::made`). `map` is the shadow map
/// as its pass draws it, with the gate's stencil (`MASKED_DEPTH`), and `map_depth_view` is its
/// depth alone, as the lit passes read it. `world` is the world's depth from the light as the
/// light's cast leaves it, a word for each slot of the clipmap (pxl-engine `light::word`).
/// `light_cs` writes it, and `PIPELINE_LIGHT_MAP` lays it into the map from the box's corner.
struct Target {
    map: u32,
    map_view: u32,
    map_depth_view: u32,
    world: u32,
    world_view: u32,
}

pub struct Shadow {
    /// The map and the world, none while the shadows are off.
    target: Option<Target>,
    sampler: u32,
    blank_view: u32,
    blank_sampler: u32,
    /// The pipeline that lays the world's depth into the map, and the gate's pipeline, which
    /// lays it in the damaged tiles alone. `lay_group` binds the world, the corner and the
    /// light's list, whose reads keep lit the tiles the epoch has not cast yet. `mask_group`
    /// binds those and also the map's tiles' words and uniform and the marks' box
    /// (`TileGrid::new_damage_rect`). `set_groups` sets both before the frame.
    lay: Pipeline,
    mask: Pipeline,
    lay_group: Group,
    mask_group: Group,
    corner: u32,
    /// Group 0 of the casters' depth pipelines, which binds the light's frame.
    pub light_group: u32,
    light_frame: u32,
    light: [f32; FRAME_FLOATS],
    pub light_view_proj: Mat4,
    /// The light box as the world's cast reads it among the eye's words (pxl-engine
    /// `eye::LIGHT`).
    pub light_words: [u32; eye::LIGHT_WORDS],
    /// A map texel's width in units at the last aim. A fragment is offset by it along its
    /// normal, so a surface does not shadow itself.
    pub texel: f64,
    /// The light direction the map is cast from. It moves only when the real light has turned
    /// a step away from it.
    pub cast_from: Option<V3>,
    /// The number of the light frame in which the clipmap's texels are cast. It moves when the
    /// light the map is cast from, the half width, or the box's depth on its `DEPTH_STEP`
    /// grid moves, and the rays' starts move with it (`starts`).
    pub generation: u32,
    key: Option<(V3, f64, i64)>,
    /// What `starts` works the rays' starts out from: the box's corner, the light's inverse
    /// and the eye's world position, as `aim` last set them. `anchors` holds the anchors
    /// worked out so far, after the number of the light frame they belong to.
    box_corner: [i32; 2],
    inverse: Mat4,
    eye_world: V3,
    anchors: (u32, Vec<Anchor>),
}

impl Shadow {
    pub fn new(g: &Gpu, pipelines: &mut PipelineCache) -> Shadow {
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
        let lay = pipelines.build_masked(g, PIPELINE_LIGHT_MAP, depth, Stencil::Unused);
        let mask = pipelines.build_masked(g, PIPELINE_LIGHT_MASK, depth, Stencil::Mask);
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
            target: None,
            sampler,
            blank_view,
            blank_sampler,
            lay,
            mask,
            lay_group: Group::default(),
            mask_group: Group::default(),
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

    /// Makes the map and the world where `on`, the shadows' setting (`FrameInput::shadows`),
    /// and releases them where not. The first map cast after they are made is cast whole, as
    /// the light's cast is (ray/cast.rs `light_made`). It returns whether they were made or
    /// released, so the lit passes' groups are made again over the map or the blank one
    /// (`entries`).
    pub fn sync(&mut self, g: &Gpu, on: bool) -> bool {
        if on == self.target.is_some() {
            return false;
        }
        match self.target.take() {
            Some(t) => {
                for id in [t.world_view, t.world, t.map_depth_view, t.map_view, t.map] {
                    release(id);
                }
            }
            None => {
                let map = g.texture(
                    SHADOW_SIZE,
                    SHADOW_SIZE,
                    1,
                    MASKED_DEPTH,
                    1,
                    texture_usage::RENDER_ATTACHMENT | texture_usage::TEXTURE_BINDING,
                );
                let world = g.texture(
                    SHADOW_SIZE,
                    SHADOW_SIZE,
                    1,
                    Format::r32uint,
                    1,
                    texture_usage::STORAGE_BINDING | texture_usage::TEXTURE_BINDING,
                );
                let world_view = g.view(world, true, 0, 1, 0, 1);
                self.target = Some(Target {
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

    /// Sets the lay's group and the gate's over the map's `tiles` and the light's list `reads`
    /// (ray/cast.rs `light_read`), before the frame's commands. It sets them where both the
    /// map and the list are made, and releases them where either is not.
    pub fn set_groups(&mut self, g: &Gpu, tiles: &TileGrid, reads: Option<u32>) {
        let (Some(t), Some(reads)) = (&self.target, reads) else {
            self.lay_group.release();
            self.mask_group.release();
            return;
        };
        let lay = [
            view_entry(0, t.world_view),
            buf_entry(1, self.corner),
            buf_entry(2, reads),
        ];
        self.lay_group.set(g, self.lay.layouts[0], &lay);
        let (words, uniform) = tiles.this_frame();
        self.mask_group.set(
            g,
            self.mask.layouts[0],
            &[
                lay[0],
                lay[1],
                lay[2],
                buf_entry(30, words),
                buf_entry(31, uniform),
                buf_entry(33, tiles.new_damage_rect()),
            ],
        );
    }

    /// Whether the map and the world are made (`made`).
    pub fn on(&self) -> bool {
        self.target.is_some()
    }

    /// The view of the world's depth from the light, which the light's cast writes; 0 while
    /// the shadows are off.
    pub fn world(&self) -> u32 {
        self.target.as_ref().map_or(0, |t| t.world_view)
    }

    fn target(&self) -> &Target {
        self.target.as_ref().or_bug(Errno::ENOTRECOVERABLE)
    }

    /// The light's frame, which a draw from the light reads in group 0, and the map as its
    /// pass draws it. A mark drawn from the light binds the first and attaches the second
    /// (pass/skinned_mesh.rs `draw_marks`).
    pub fn frame_and_map(&self) -> (u32, u32) {
        (self.light_frame, self.target().map_view)
    }

    /// The map and its sampler at `binding` and the next binding, as a lit pass reads them
    /// (shadow.wgsl). While the shadows are off, they are the blank map's.
    pub fn entries(&self, binding: u32) -> [Entry; 2] {
        match &self.target {
            Some(t) => [
                view_entry(binding, t.map_depth_view),
                sampler_entry(binding + 1, self.sampler),
            ],
            None => self.blank_entries(binding),
        }
    }

    /// The blank map and its sampler, for a pass that has no shadow map to read. No pass draws
    /// the blank map, and every face reads it as unshadowed.
    pub fn blank_entries(&self, binding: u32) -> [Entry; 2] {
        [
            view_entry(binding, self.blank_view),
            sampler_entry(binding + 1, self.blank_sampler),
        ]
    }

    /// Aims the light box at `target` along the light `sun_dir`, which is the sun's or the
    /// moon's direction. `half_width` is the map's half width about `target`, and a receiver
    /// reads the map within the share `reach_ndc` of it. `fixed` is a point that stays put in
    /// the world as the eye moves, in the eye's frame.
    pub fn aim(&mut self, sun_dir: &[f32], target: V3, half_width: f64, reach_ndc: f32, fixed: V3) {
        self.texel = half_width * 2.0 / f64::from(SHADOW_SIZE);
        // The light the map is cast from turns only in steps, each of which turns the map's
        // edge by a texel, so a shadow moves on a texel at a time as the light goes. A light
        // turned every frame would snap the grid below another way each time, and a wall's
        // shadow would flicker.
        let light = v3::norm(&[sun_dir[0] as f64, sun_dir[1] as f64, sun_dir[2] as f64]);
        let step = self.texel / half_width;
        let from = match self.cast_from {
            Some(from) if v3::dot(&from, &light) > libm::cos(step) => from,
            _ => light,
        };
        self.cast_from = Some(from);
        // The box's eye stands two half widths up the light from the target, and its near and
        // far planes lie a half width either side of the target. The sphere of radius
        // `half_width` about the target then lies between them whatever the light's angle. A
        // margin lies past each plane, the two together `DEPTH_STEP` of the box's depth.
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
        // The map follows the eye, so its texels are slid onto a grid fixed in the world
        // (stable shadow maps, texel snapping). Its planes are slid onto a coarser grid along
        // the light, so a texel that is kept still holds the same ray. Where the fixed point
        // falls on the texels' grid gives the world texel index of the box's first texel, X
        // across and Y down the rows. That index is the clipmap's corner. It is a whole number
        // of tiles (`light::TILE`), so a tile of the box holds whole tiles of the world, and
        // what the cast read of it moves with the box a tile at a time (light_slide.wgsl).
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
        // The world's cast reads the box from the eye's words, as the inverse of the light's
        // view and projection, which takes the map's NDC to the eye's frame.
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
            SHADOW_SIZE,
            corner,
            reach_ndc,
        )
        .words();
        (self.box_corner, self.inverse) = (corner, inverse);
        self.eye_world = fixed.map(|v| -v);
    }

    /// Where the world's cast starts each texel's ray (pxl-engine `light::StartGrid`): the four
    /// anchors about the box's corner, and a texel's steps across and down in Q3 steps. An
    /// anchor is where its world texel's middle lies on the near plane, in Q3 from the eye's
    /// world position. Each anchor is worked out once in a light frame and kept while the frame
    /// holds (`anchors`), so a texel's start is the same in every frame cast in that light
    /// frame.
    pub fn starts(&mut self) -> StartGrid {
        if self.anchors.0 != self.generation {
            self.anchors = (self.generation, Vec::new());
        }
        let corner = self.box_corner;
        let base = [
            corner[0].div_euclid(ANCHOR) * ANCHOR,
            corner[1].div_euclid(ANCHOR) * ANCHOR,
        ];
        let q3 = f64::from(1u32 << FRAC_W);
        let texel = 2.0 / f64::from(SHADOW_SIZE);
        let (inverse, eye) = (self.inverse, self.eye_world);
        let mut anchor = |k: usize| {
            let w = [
                base[0].wrapping_add((k % 2) as i32 * ANCHOR),
                base[1].wrapping_add((k / 2) as i32 * ANCHOR),
            ];
            if let Some(&(_, at, frac)) = self.anchors.1.iter().find(|a| a.0 == w) {
                return (at, frac);
            }
            // The anchor's middle is taken in the map's NDC on the near plane, as `Light::of`
            // lays the box's texels. The light's inverse takes it to the eye's frame, and the
            // eye's world position then to the world.
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
        StartGrid {
            base,
            across: light.across.map(|v| light::fixed(v * q3 as f32)),
            down: light.down.map(|v| light::fixed(v * q3 as f32)),
            at: [a0.0, a1.0, a2.0, a3.0],
            frac: [a0.1, a1.1, a2.1, a3.1],
        }
    }

    /// Puts the eye's Q3 into the light's frame as into the eye's, so a draw from the light
    /// takes its position against the same eye (frame.wgsl `from_eye`).
    pub fn put_eye(&mut self, eye: [f32; 3]) {
        self.light[EYE..EYE + 3].copy_from_slice(&eye);
    }

    /// Puts the wind's drift into the light's frame as into the eye's, so a cloud drawn from
    /// the light is moved back by the same drift (instance.wgsl `draw_translation`).
    pub fn put_drift(&mut self, drift: &[f32]) {
        self.light[DRIFT_Q..DRIFT_Q + 4].copy_from_slice(drift);
    }

    pub fn write(&self, g: &Gpu) {
        g.write_buf(self.light_frame, 0, bytes(&self.light));
    }

    /// Begins the map's pass, timed in the span `shadow`. The caller draws the casters into it
    /// through the stencil and then ends it. A pass drawn whole clears the map to the far
    /// plane. Where `cast` says the light's cast holds this frame's world, it first lays the
    /// world's depth in the tiles the light's cast last listed as read, a quad a tile, as many
    /// as the cast opened (`light::ARGS_LAY`). Every texel a receiver reads lies in one of
    /// those tiles, and no receiver reads the far plane the rest hold. A pass drawn through its
    /// gate (`masked`) draws the damaged tiles of `tiles` alone and keeps the rest as the last
    /// frame left them; a gated frame always casts. The gate lays the world in the damaged
    /// tiles and raises their stencil to the number `TileGrid::mask` gives: a quad a tile of the
    /// box the frame's marks cover, with the undamaged ones collapsed, as many as the GPU
    /// counted (`TileGrid::rect_args`). Both draws bind the groups `set_groups` set before the
    /// frame.
    pub fn begin<'a>(
        &self,
        ctx: &mut RenderContext<'a>,
        cast: bool,
        tiles: &mut TileGrid,
        masked: bool,
        LightRead { epoch, args, .. }: LightRead,
    ) -> TrackedRenderPass<'a> {
        let (g, encoder) = (ctx.g, ctx.encoder);
        let t = self.target();
        // The lay's uniform (light_lay.wgsl `Lay`) holds the box's corner, from which the
        // world's depth is laid in, and the epoch and the reads, which tell the tiles the
        // light's epoch has cast from those it has not, counting from where a slide went
        // unrun. It also holds where the list of the tiles read lies.
        let corner = [
            self.light_words[light::CORNER[0]],
            self.light_words[light::CORNER[1]],
            epoch,
            light::reads_at(SHADOW_SIZE),
            light::list_cnt_at(SHADOW_SIZE, light::LIST_READS),
            light::slide_at(SHADOW_SIZE) + light::READS_FROM,
            0,
            0,
        ];
        g.write_buf(self.corner, 0, bytes(&corner));
        let stamp = ctx.stamp(GpuSpanName::shadow);
        let load = if masked { Load::Load } else { Load::Clear };
        let desc = pass_desc(0, Load::Clear, t.map_view, load, 1.0, stamp.pass());
        let (desc, number) = tiles.mask(desc, masked);
        let pass = g.pass(encoder, &desc);
        g.declare_pass(pass, sequence::MAP_PASS);
        let mut pass = TrackedRenderPass::new(g, pass);
        if let Some(n) = number {
            xpute_kit::ensure!(cast, EINVAL, n);
            pass.set_stencil_reference(n);
            pass.set_pipeline(self.mask.id);
            pass.set_bind_group(0, self.mask_group.id(), &[]);
            if let Some(args) = tiles.rect_args() {
                pass.draw_indirect(args, sequence::MAP_MASK.at());
            }
        } else if cast {
            pass.set_pipeline(self.lay.id);
            pass.set_bind_group(0, self.lay_group.id(), &[]);
            pass.draw_indirect(args, sequence::MAP_LAY.at());
        }
        pass
    }
}
