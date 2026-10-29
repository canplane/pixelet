// pxl-render/gpu/shadow.rs

//! The sun's shadow map: the world's depth from the sun each frame,
//! orthographic about the eye and as wide as the casters reach (the
//! kernel's `STAGE_AT::SHADOWS`), so its texel follows the camera grid.
//! The world is cast a ray a texel through the world's query, as the eye's
//! rays are, off samples and the visible sample (pxl-shader `sun_cs`), into
//! `world`, a clipmap the light box slides over a texel at a time, every
//! texel each frame it is cast (pxl-spec-core `sun`); laid into the map
//! each frame in the box's own order
//! (`PIPELINE_SUN_MAP`), and decor and the actors, which move every frame,
//! drawn over it depth only, from the light's frame. The lit passes read
//! the map as a plain light box. 16 MB each at 2048.

use pxl_spec::space::coord::mat4::{self, Mat4};
use pxl_spec::space::coord::v3::{self, V3};
use pxl_spec_core::eye;
use pxl_spec_core::sun::{self, Starts, Sun, ANCHOR};
use xpute_gpu::enums::{AddressMode, CompareFunction, FilterMode, MipmapFilterMode};

use crate::frame::shader::record::{PIPELINE_DECOR_DEPTH, PIPELINE_SUN_MAP};
use crate::frame::shader::word::frame::VIEW_PROJ;
use crate::frame::shader::word::FRAME_FLOATS;
use crate::gpu::api::{
    buf_entry, buffer_usage, bytes, pass_desc, sampler_entry, texture_usage, view_entry,
    word_bytes, Entry, Format, Gpu, Load, Stamp,
};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::put::put_mat;
use crate::param::shadow::DEPTH_STEP;

/// The map's side in texels.
pub const SIZE: u32 = 2048;
const _: () = assert!(
    SIZE as i32 <= ANCHOR,
    "the box lies in the four anchors about its corner (`Shadow::starts`)"
);

/// An anchor of the rays' starts (`Shadow::starts`): its world texel, its
/// start in Q3 and what rounding to it left.
type Anchor = ([i32; 2], [i32; 3], [f32; 3]);

/// WebGPU clip z in [0, 1], column-major.
fn ortho(out: &mut Mat4, half: f64, near: f64, far: f64) {
    *out = [0.0; 16];
    out[0] = 1.0 / half;
    out[5] = 1.0 / half;
    out[10] = -1.0 / (far - near);
    out[14] = -near / (far - near);
    out[15] = 1.0;
}

pub struct Shadow {
    map_view: u32,
    sampler: u32,
    blank_view: u32,
    blank_sampler: u32,
    /// The world's depth from the sun as its cast leaves it, a word a slot
    /// of the clipmap (pxl-spec-core `sun::Sun::word`): `sun_cs` writes it,
    /// `PIPELINE_SUN_MAP` lays it into the map off the box's corner,
    /// `corner`'s words.
    pub world: u32,
    lay: Built,
    lay_group: u32,
    corner: u32,
    /// Group 0 of the casters' depth pipelines: the light's frame.
    pub light_group: u32,
    light_frame: u32,
    light: [f32; FRAME_FLOATS],
    pub light_view_proj: Mat4,
    /// The world's cast's words off it (pxl-spec-core `eye::SUN`).
    pub sun: [u32; eye::SUN_WORDS],
    /// The normal offset a fragment takes: a texel at the last reach.
    pub texel: f64,
    /// The sun the map is cast from, moved only when the real one has gone a
    /// step from it.
    pub cast_from: Option<V3>,
    /// The light's frame the clipmap's texels are cast in, numbered: moved
    /// where the sun the map is cast from, the reach or the depth step
    /// (`DEPTH_STEP`) moved, the rays' starts with it (`starts`).
    pub frame: u32,
    held: Option<(V3, f64, i64)>,
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
        let map = g.texture(
            SIZE,
            SIZE,
            1,
            Format::depth32float,
            1,
            texture_usage::RENDER_ATTACHMENT | texture_usage::TEXTURE_BINDING,
        );
        let map_view = g.view(map, false, 0, 1, 0, 1);
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
        let world = g.texture(
            SIZE,
            SIZE,
            1,
            Format::r32uint,
            1,
            texture_usage::STORAGE_BINDING | texture_usage::TEXTURE_BINDING,
        );
        let world = g.view(world, true, 0, 1, 0, 1);
        let lay = pipelines.build(g, PIPELINE_SUN_MAP, Format::depth32float);
        let corner = g.buf(
            word_bytes(4),
            buffer_usage::UNIFORM | buffer_usage::COPY_DST,
        );
        let lay_group = g.bind_group(
            lay.layouts[0],
            &[view_entry(0, world), buf_entry(1, corner)],
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
            map_view,
            sampler,
            blank_view,
            blank_sampler,
            world,
            lay,
            lay_group,
            corner,
            light_group,
            light_frame,
            light: [0.0; FRAME_FLOATS],
            light_view_proj: mat4::new(),
            sun: [0; eye::SUN_WORDS],
            texel: 0.0,
            cast_from: None,
            frame: 0,
            held: None,
            box_corner: [0; 2],
            inverse: mat4::new(),
            eye_world: [0.0; 3],
            anchors: (u32::MAX, Vec::new()),
        }
    }

    /// The map and its sampler, at `binding` and the next, as a lit pass reads them (shadow.wgsl).
    pub fn entries(&self, binding: u32) -> [Entry; 2] {
        [
            view_entry(binding, self.map_view),
            sampler_entry(binding + 1, self.sampler),
        ]
    }

    /// A map no pass draws, for a pass that draws none: every face reads it unshadowed.
    pub fn blank_entries(&self, binding: u32) -> [Entry; 2] {
        [
            view_entry(binding, self.blank_view),
            sampler_entry(binding + 1, self.blank_sampler),
        ]
    }

    /// `reach` is the map's half width about `target`, `rim` the share of
    /// it a receiver reads within; `fixed` is a point that stays put in the
    /// world as the eye moves, in the eye's frame.
    pub fn aim(&mut self, sun_dir: &[f32], target: V3, reach: f64, rim: f32, fixed: V3) {
        self.texel = reach * 2.0 / f64::from(SIZE);
        // A step turns the map's edge by a texel, so a shadow ticks a texel on as
        // the sun goes, on purpose: the sun read as moving. Turned on every
        // frame instead, the grid below snapped another way each time and a
        // wall's shadow flickered as fast as the frames came.
        let sun = v3::norm(&[sun_dir[0] as f64, sun_dir[1] as f64, sun_dir[2] as f64]);
        let step = self.texel / reach;
        let from = match self.cast_from {
            Some(from) if v3::dot(&from, &sun) > libm::cos(step) => from,
            _ => sun,
        };
        self.cast_from = Some(from);
        // Two reaches up the sun, so the sphere of `reach` about the target lies
        // between the planes a reach either side of it, whatever the sun's
        // angle, and a share past them each side (`DEPTH_STEP`).
        let eye: V3 = core::array::from_fn(|k| target[k] + from[k] * 2.0 * reach);
        let (mut view, mut proj) = (mat4::new(), mat4::new());
        mat4::look_at(&mut view, eye, target, [0.0, 0.0, 1.0]);
        let margin = reach * DEPTH_STEP / (1.0 - DEPTH_STEP);
        ortho(&mut proj, reach, reach - margin, 3.0 * reach + margin);
        mat4::mul(&mut self.light_view_proj, &proj, &view);
        // The map follows the eye, so its texels are slid onto a grid fixed in the
        // world: off it, every edge fell on other texels each frame as the view
        // moved, and shimmered (stable shadow maps, texel snapping); and its
        // planes onto a coarser one along the sun, so a texel kept is the
        // same ray. Where the fixed point falls on the texels' grid is the
        // world's texel index of the box's first texel, X across and Y down
        // the rows: the clipmap's corner.
        let m = &mut self.light_view_proj;
        let mut cell = [0i64; 3];
        for (k, step) in [(0, step), (1, step), (2, DEPTH_STEP)] {
            let at = m[k] * fixed[0] + m[4 + k] * fixed[1] + m[8 + k] * fixed[2] + m[12 + k];
            let n = (at / step).round_ties_even();
            m[12 + k] -= at - n * step;
            cell[k] = n as i64;
        }
        let corner = [cell[0].wrapping_neg() as i32, cell[1] as i32];
        let held = Some((from, reach, cell[2]));
        if held != self.held {
            (self.frame, self.held) = (self.frame.wrapping_add(1), held);
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
        self.sun = Sun::of(
            [column(0), column(1), column(2), column(3)],
            SIZE,
            corner,
            rim,
        )
        .words();
        (self.box_corner, self.inverse) = (corner, inverse);
        self.eye_world = fixed.map(|v| -v);
    }

    /// Where the world's cast starts each texel's ray (pxl-spec-core
    /// `sun::Starts`): the anchors about the box's corner, each worked out
    /// once a light frame, on the near plane where its world texel's middle
    /// lies, in Q3 off the eye's world position, and kept while the frame
    /// holds (`anchors`), so a texel's start is the same in every frame
    /// cast in it; and a texel's steps, in Q3 steps.
    pub fn starts(&mut self) -> Starts {
        if self.anchors.0 != self.frame {
            self.anchors = (self.frame, Vec::new());
        }
        let corner = self.box_corner;
        let base = [
            corner[0].div_euclid(ANCHOR) * ANCHOR,
            corner[1].div_euclid(ANCHOR) * ANCHOR,
        ];
        let q3 = f64::from(1u32 << pxl_spec_core::FRAC_W);
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
            // Its middle's NDC on the near plane, as `Sun::of` lays the box's
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
            let (at, frac): (Vec<i32>, Vec<f32>) = (0..3)
                .map(|a| {
                    let v = p(a);
                    let r = v.round_ties_even();
                    ((r as i64) as i32, (v - r) as f32)
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
        let sun = Sun::of_words(&self.sun);
        Starts {
            base,
            across: sun.across.map(|v| v * q3 as f32),
            down: sun.down.map(|v| v * q3 as f32),
            at: [a0.0, a1.0, a2.0, a3.0],
            frac: [a0.1, a1.1, a2.1, a3.1],
        }
    }

    /// The light's frame, where the casters' depth pipelines read their
    /// view, and the box's corner the world's depth is laid in off.
    pub fn write(&self, g: &Gpu) {
        g.write_buf(self.light_frame, 0, bytes(&self.light));
        let corner = [self.sun[sun::CORNER[0]], self.sun[sun::CORNER[1]], 0, 0];
        g.write_buf(self.corner, 0, bytes(&corner));
    }

    /// The map's pass, timed in `stamp`, the world's depth laid in first
    /// where `cast` (its cast holds this frame's world); else cleared to the
    /// far plane, as nothing is cast there. Where only the texels `rect`
    /// holds, `[x, y, width, height]`, are drawn again, the rest kept as the
    /// last frame left it, the pass is held to them, the world laid there
    /// first (`PIPELINE_SUN_MAP` writes every texel it covers), which a
    /// frame that casts alone may. The casters are drawn into it next, then
    /// it ends.
    pub fn begin(
        &self,
        g: &Gpu,
        encoder: u32,
        cast: bool,
        rect: Option<[u32; 4]>,
        stamp: &mut Stamp,
    ) -> u32 {
        let load = if rect.is_some() {
            Load::Load
        } else {
            Load::Clear
        };
        let pass = g.pass(
            encoder,
            &pass_desc(0, Load::Clear, self.map_view, load, 1.0, stamp.pass()),
        );
        if let Some(r) = rect {
            g.set_scissor(pass, r);
        }
        if cast {
            g.set_pipeline(pass, self.lay.pipeline);
            g.set_bind_group(pass, 0, self.lay_group, &[]);
            g.draw(pass, 3, 1, 0, 0);
        }
        pass
    }
}
