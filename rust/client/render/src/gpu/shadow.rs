// pxl-render/gpu/shadow.rs

//! The light's shadow map: the world's depth from the light each frame,
//! orthographic about the eye, in clip levels (pxl-spec-core
//! `light::LEVELS`): level `l` a box `2^l` times as wide as the first at
//! the same side in texels, so its texel follows the camera grid, coarser
//! farther off as the cells are, and the coarsest reaching as far as the
//! eye draws. Each level is cast as the one map was: a ray a texel through
//! the world's query, as the eye's rays are, off samples and the visible
//! sample (pxl-shader `light_cs`), into its layer of `world`, a clipmap the
//! level's box slides over a texel at a time, every texel each frame it is
//! cast (pxl-spec-core `light`); laid into its layer of the map each frame
//! in the box's own order, one triangle over it (`PIPELINE_LIGHT_MAP`), and
//! decor and the actors, which move every frame, drawn over it depth only,
//! from the level's frame. The lit passes read the map as a plain light box
//! a level, the finest that holds the receiver (shadow.wgsl). Where only the
//! listed casters that changed moved over the world its cast left as it
//! was, a level is drawn again in the tiles they set alone (gpu/tiles.rs),
//! through a gate of its own (light_gate.wgsl), as the world's pass is.

use pxl_spec::space::camera::grid::split_dist;
use pxl_spec::space::coord::mat4::{self, Mat4};
use pxl_spec::space::coord::v3::{self, V3};
use pxl_spec::space::layer::lattice;
use pxl_spec::space::layer::tile::TOP_ZOOM;
use pxl_spec_core::grid;
use pxl_spec_core::light::{self, Light, Starts, ANCHOR};
use xpute_gpu::enums::{AddressMode, CompareFunction, FilterMode, MipmapFilterMode};

use crate::frame::shader::floor;
use crate::frame::shader::record::{
    PIPELINE_DECOR_DEPTH, PIPELINE_LIGHT_GATE, PIPELINE_LIGHT_MAP, SHADOW_LEVELS_BINDING,
};
use crate::frame::shader::word::frame::VIEW_PROJ;
use crate::frame::shader::word::FRAME_FLOATS;
use crate::gpu::api::{
    buf_entry, buffer_usage, bytes, pass_desc, release, sampler_entry, texture_usage, view_entry,
    word_bytes, Entry, Format, Gpu, Load, RenderContext, Stencil, TrackedRenderPass,
};
use crate::gpu::pipeline::{Pipeline, Pipelines};
use crate::gpu::put::put_mat;
use crate::gpu::span_spec::GpuSpanName;
use crate::gpu::tiles::Tiles;
use crate::param::shadow::DEPTH_STEP;

/// The first level's side in texels, and every other level's: the first a
/// texel of `FIRST_TEXEL` out to where the ground's leaf cells end, as the
/// one map was before the levels, so no shadow is coarser than it was
/// anywhere it was cast; past it each level twice as wide, its texel no
/// wider than the camera grid's cell at the level's near edge, the least
/// side that keeps it so (asserted below).
pub const FIRST_SIZE: u32 = 2048;
pub const SIZE: u32 = 512;

/// Level `l`'s side in texels.
pub const fn size(l: u32) -> u32 {
    if l == 0 {
        FIRST_SIZE
    } else {
        SIZE
    }
}

const _: () = assert!(
    FIRST_SIZE as i32 <= ANCHOR && SIZE <= FIRST_SIZE,
    "the box lies in the four anchors about its corner (`Shadow::starts`)"
);
const _: () = assert!(
    light::tiles(FIRST_SIZE) * light::tiles(FIRST_SIZE) <= light::MISSED_TILES,
    "a listed slot's walk names its tile in the bits over `light::MISS_TILE_SHIFT` (`light::miss_word`)"
);

/// The first level's texel, in units: an eighth of the ground's leaf cell.
pub const FIRST_TEXEL: f64 = 1.0 / 8.0;

/// Level `l`'s half-width, in units: the first's twice over `l` times.
pub const fn half_width(l: u32) -> f64 {
    FIRST_TEXEL * (FIRST_SIZE / 2) as f64 * (1u64 << l) as f64
}

/// Level `l`'s texel, in units.
pub const fn texel(l: u32) -> f64 {
    2.0 * half_width(l) / size(l) as f64
}

/// The share of a level's half-width a receiver reads it within, every
/// level's alike so each reaches twice as far as the one before
/// (pxl-spec-core `light::level_of`): the smaller side's.
pub const READ_SHARE: f32 = light::read_share(SIZE);

/// How far from the eye across the light's rays a receiver reads level `l`,
/// in units.
pub const fn reach(l: u32) -> f64 {
    READ_SHARE as f64 * half_width(l)
}

/// Whether every level past the first has a texel no wider than the camera
/// grid's cell where its receivers begin, the last level's reach.
const fn follows_grid() -> bool {
    let mut l = 1;
    while l < light::LEVELS {
        if texel(l) > reach(l - 1) * grid::CELL as f64 {
            return false;
        }
        l += 1;
    }
    true
}

const _: () = assert!(
    reach(0) >= split_dist(lattice::GROUND.cell() - 1) && texel(0) == FIRST_TEXEL,
    "the first level reads at `FIRST_TEXEL` out to where the ground's leaf cells end"
);
const _: () = assert!(
    follows_grid(),
    "a level's texel is no wider than the camera grid's cell at its near edge"
);
const _: () = assert!(
    reach(light::LEVELS - 1) >= lattice::draw_dist(lattice::GROUND, TOP_ZOOM),
    "the coarsest level reaches as far as the eye draws at the pyramid's top"
);

/// Where level `l`'s list lies (pxl-spec-core `light::list_words`): the
/// first level's in a buffer of its own, the rest's in one, a level's at
/// `light::level_words` apart; which, and its first word there.
pub const fn list_at(l: u32) -> (usize, u32) {
    if l == 0 {
        (0, 0)
    } else {
        (1, (l - 1) * light::level_words(SIZE))
    }
}

/// The words of each of the two buffers the levels' lists lie in
/// (`list_at`).
pub const LIST_WORDS: [u32; 2] = [
    light::list_words(FIRST_SIZE),
    (light::LEVELS - 1) * light::level_words(SIZE),
];

const _: () = assert!(
    floor::fits(4 * LIST_WORDS[0] as u64, true) && floor::fits(4 * LIST_WORDS[1] as u64, true),
    "each buffer of the levels' lists is one every device binds whole"
);

/// The first word of level `l`'s stands among the levels' one buffer
/// (pxl-spec-core `light::stands`), and every level's past the last.
pub const fn stands_at(l: u32) -> u32 {
    if l == 0 {
        0
    } else {
        light::stands(FIRST_SIZE) + (l - 1) * light::stands(SIZE)
    }
}

/// The levels the map casts where the eye draws `far` units out: the
/// finest whose reach holds it and every one finer, a receiver no farther
/// out for the levels than it lies from the eye (pxl-spec-core
/// `Light::level_dist`).
pub fn levels_for(far: f64) -> u32 {
    (light::level_of(far as f32, reach(0) as f32) + 1).min(light::LEVELS)
}

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

/// A level of the map: its frame, the world's cast's words off it and the
/// rays' starts, what it is laid by, and its own views of the map and of
/// the world.
pub struct Level {
    /// Its layer of the map as its pass draws it, and of the world's depth
    /// from the light as its cast writes it, a word a slot of its clipmap
    /// (pxl-spec-core `light::word`, `light_cs`); the world's array it lies
    /// in, which the lay reads.
    map_view: u32,
    pub world: u32,
    world_array: u32,
    /// The box's corner and its layer of the world laid off (light_lay.wgsl).
    corner: u32,
    lay_group: u32,
    gate_group: u32,
    gate_bound: (u32, u32, u32),
    /// Group 0 of the casters' depth pipelines: the level's frame.
    pub light_group: u32,
    light_frame: u32,
    light: [f32; FRAME_FLOATS],
    pub light_view_proj: Mat4,
    /// The world's cast's words off it (pxl-spec-core `light::Light::words`).
    pub light_words: [u32; light::WORDS],
    /// The light's frame its clipmap's texels are cast in, numbered: moved
    /// where the light the map is cast from, its box's place along the rays
    /// or the depth step (`DEPTH_STEP`) moved, the rays' starts with it
    /// (`starts`).
    pub generation: u32,
    key: Option<(V3, i64)>,
    /// What the rays' starts are worked out off (`starts`): the box's
    /// corner and the light's inverse, as last aimed; and the anchors worked
    /// out in the light frame numbered first.
    box_corner: [i32; 2],
    inverse: Mat4,
    anchors: (u32, Vec<Anchor>),
}

pub struct Shadow {
    /// The map, the first level's and the rest's a layer each, with the
    /// gate's stencil where the device carries one (`Gpu::gated_depth`),
    /// their depth alone as the lit passes read it.
    first_depth_view: u32,
    levels_depth_view: u32,
    sampler: u32,
    blank_view: u32,
    blank_array_view: u32,
    blank_sampler: u32,
    lay: Pipeline,
    gate: Pipeline,
    pub levels: Vec<Level>,
    /// The levels cast and read this frame (`levels_for`).
    pub level_cnt: u32,
    /// The light the map is cast from, moved only when the real one has gone a
    /// step from it.
    pub cast_from: Option<V3>,
    /// The eye's world position, as last aimed, which the starts are off.
    eye_world: V3,
}

impl Shadow {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines) -> Shadow {
        let rest = light::LEVELS - 1;
        let target = texture_usage::RENDER_ATTACHMENT | texture_usage::TEXTURE_BINDING;
        let first = g.texture(FIRST_SIZE, FIRST_SIZE, 1, g.gated_depth(), 1, target);
        let maps = g.texture(SIZE, SIZE, rest, g.gated_depth(), 1, target);
        let first_depth_view = g.depth_only_view(first);
        let levels_depth_view = g.depth_only_array_view(maps, rest);
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
        let blank_array_view = g.view(blank, true, 0, 1, 0, 1);
        let blank_sampler = g.sampler(
            FilterMode::nearest,
            FilterMode::nearest,
            MipmapFilterMode::nearest,
            AddressMode::clamp_to_edge,
            Some(CompareFunction::less_equal),
            1,
        );
        let storage = texture_usage::STORAGE_BINDING | texture_usage::TEXTURE_BINDING;
        let first_world = g.texture(FIRST_SIZE, FIRST_SIZE, 1, Format::r32uint, 1, storage);
        let worlds = g.texture(SIZE, SIZE, rest, Format::r32uint, 1, storage);
        let first_world_array = g.view(first_world, true, 0, 1, 0, 1);
        let worlds_array = g.view(worlds, true, 0, 1, 0, rest);
        let depth = Format::depth32float;
        let lay = pipelines.build_gated(g, PIPELINE_LIGHT_MAP, depth, Stencil::Unused);
        let gate = pipelines.build_gated(g, PIPELINE_LIGHT_GATE, depth, Stencil::Gate);
        let depth_layout = pipelines.build(g, PIPELINE_DECOR_DEPTH, g.canvas()).layouts[0];
        let levels = (0..light::LEVELS)
            .map(|l| {
                let corner = g.buf(
                    word_bytes(4),
                    buffer_usage::UNIFORM | buffer_usage::COPY_DST,
                );
                let light_frame = g.buf(
                    word_bytes(FRAME_FLOATS),
                    buffer_usage::UNIFORM | buffer_usage::COPY_DST,
                );
                let (map_view, world, world_array) = if l == 0 {
                    (
                        g.view(first, false, 0, 1, 0, 1),
                        first_world_array,
                        first_world_array,
                    )
                } else {
                    (
                        g.view(maps, false, 0, 1, l - 1, 1),
                        g.view(worlds, true, 0, 1, l - 1, 1),
                        worlds_array,
                    )
                };
                Level {
                    map_view,
                    world,
                    world_array,
                    corner,
                    lay_group: g.bind_group(
                        lay.layouts[0],
                        &[view_entry(0, world_array), buf_entry(1, corner)],
                    ),
                    gate_group: 0,
                    gate_bound: (0, 0, 0),
                    light_group: g.bind_group(depth_layout, &[buf_entry(0, light_frame)]),
                    light_frame,
                    light: [0.0; FRAME_FLOATS],
                    light_view_proj: mat4::new(),
                    light_words: [0; light::WORDS],
                    generation: 0,
                    key: None,
                    box_corner: [0; 2],
                    inverse: mat4::new(),
                    anchors: (u32::MAX, Vec::new()),
                }
            })
            .collect();
        Shadow {
            first_depth_view,
            levels_depth_view,
            sampler,
            blank_view,
            blank_array_view,
            blank_sampler,
            lay,
            gate,
            levels,
            level_cnt: 0,
            cast_from: None,
            eye_world: [0.0; 3],
        }
    }

    /// Level `l`'s frame, which a draw from the light reads in group 0, and
    /// its layer of the map as its pass draws it: what a mark from the light
    /// binds and attaches (gpu/pass/skinned_mesh.rs `draw_marks`).
    pub fn frame_and_map(&self, l: u32) -> (u32, u32) {
        let v = &self.levels[l as usize];
        (v.light_frame, v.map_view)
    }

    /// The map's first level and its sampler, at `binding` and the next,
    /// and the rest's layers at `SHADOW_LEVELS_BINDING`, as a lit pass reads them
    /// (shadow.wgsl).
    pub fn entries(&self, binding: u32) -> [Entry; 3] {
        [
            view_entry(binding, self.first_depth_view),
            sampler_entry(binding + 1, self.sampler),
            view_entry(SHADOW_LEVELS_BINDING, self.levels_depth_view),
        ]
    }

    /// A map no pass draws, for a pass that draws none: every face reads it unshadowed.
    pub fn blank_entries(&self, binding: u32) -> [Entry; 3] {
        [
            view_entry(binding, self.blank_view),
            sampler_entry(binding + 1, self.blank_sampler),
            view_entry(SHADOW_LEVELS_BINDING, self.blank_array_view),
        ]
    }

    /// The levels as far as the eye draws `far` units out (`levels_for`),
    /// each aimed about `target` (`aim_level`); `fixed` is a point that
    /// stays put in the world as the eye moves, in the eye's frame.
    pub fn aim(&mut self, sun_dir: &[f32], target: V3, far: f64, fixed: V3) {
        // A step turns the map's edge by a texel, so a shadow ticks a texel on as
        // the light goes, on purpose: the light read as moving. Turned on every
        // frame instead, the grid below snapped another way each time and a
        // wall's shadow flickered as fast as the frames came. A texel over its
        // half-width is the same at every level.
        let light = v3::norm(&[sun_dir[0] as f64, sun_dir[1] as f64, sun_dir[2] as f64]);
        let step = 2.0 / f64::from(FIRST_SIZE);
        let from = match self.cast_from {
            Some(from) if v3::dot(&from, &light) > libm::cos(step) => from,
            _ => light,
        };
        self.cast_from = Some(from);
        self.level_cnt = levels_for(far);
        self.eye_world = fixed.map(|v| -v);
        for l in 0..self.level_cnt {
            self.aim_level(l, from, target, fixed);
        }
    }

    /// Level `l`'s box, `half_width(l)` about `target`, cast from `from`.
    fn aim_level(&mut self, l: u32, from: V3, target: V3, fixed: V3) {
        let half_width = half_width(l);
        let step = 2.0 / f64::from(size(l));
        let v = &mut self.levels[l as usize];
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
        mat4::mul(&mut v.light_view_proj, &proj, &view);
        // The map follows the eye, so its texels are slid onto a grid fixed in the
        // world: off it, every edge fell on other texels each frame as the view
        // moved, and shimmered (stable shadow maps, texel snapping); and its
        // planes onto a coarser one along the light, so a texel kept is the
        // same ray. Where the fixed point falls on the texels' grid is the
        // world's texel index of the box's first texel, X across and Y down
        // the rows: the clipmap's corner, a whole number of tiles
        // (`light::TILE`), so a tile of the box holds whole tiles of the world
        // and what the cast read of it moves with the box a tile at a time
        // (pxl-shader light.rs `light_detail_cs`).
        let m = &mut v.light_view_proj;
        let mut cell = [0i64; 3];
        let tile = f64::from(light::TILE);
        for (k, step, whole) in [(0, step, tile), (1, step, tile), (2, DEPTH_STEP, 1.0)] {
            let at = m[k] * fixed[0] + m[4 + k] * fixed[1] + m[8 + k] * fixed[2] + m[12 + k];
            let n = (at / (step * whole)).round_ties_even() * whole;
            m[12 + k] -= at - n * step;
            cell[k] = n as i64;
        }
        let corner = [cell[0].wrapping_neg() as i32, cell[1] as i32];
        let key = Some((from, cell[2]));
        if key != v.key {
            (v.generation, v.key) = (v.generation.wrapping_add(1), key);
        }
        put_mat(&mut v.light, VIEW_PROJ, &v.light_view_proj);
        // The world's cast reads the box off the level's words: the light's
        // view and projection undone, the map's NDC to the eye's frame.
        let mut inverse = mat4::new();
        mat4::invert(&mut inverse, &v.light_view_proj);
        let column = |k: usize| {
            [
                inverse[4 * k] as f32,
                inverse[4 * k + 1] as f32,
                inverse[4 * k + 2] as f32,
            ]
        };
        v.light_words = Light::of(
            [column(0), column(1), column(2), column(3)],
            size(l),
            corner,
            READ_SHARE,
        )
        .words();
        (v.box_corner, v.inverse) = (corner, inverse);
    }

    /// Level `l`'s translation, the last column of its frame, which the lit
    /// passes take with the first level's turned and scaled by `2^-l`
    /// (shadow.wgsl `shadow_level_clip`): the levels' boxes share the
    /// light's turn, each `2^l` times the first's across and deep; and its
    /// texel, the normal's offset there.
    pub fn translation(&self, l: u32) -> [f32; 4] {
        let m = &self.levels[l as usize].light_view_proj;
        [m[12] as f32, m[13] as f32, m[14] as f32, texel(l) as f32]
    }

    /// Where level `l`'s cast starts each texel's ray (pxl-spec-core
    /// `light::Starts`): the anchors about the box's corner, each worked out
    /// once a light frame, on the near plane where its world texel's middle
    /// lies, in Q3 off the eye's world position, and kept while the frame
    /// holds (`anchors`), so a texel's start is the same in every frame
    /// cast in it; and a texel's steps, in Q3 steps.
    pub fn starts(&mut self, l: u32) -> Starts {
        let eye = self.eye_world;
        let v = &mut self.levels[l as usize];
        if v.anchors.0 != v.generation {
            v.anchors = (v.generation, Vec::new());
        }
        let corner = v.box_corner;
        let base = [
            corner[0].div_euclid(ANCHOR) * ANCHOR,
            corner[1].div_euclid(ANCHOR) * ANCHOR,
        ];
        let q3 = f64::from(1u32 << pxl_spec_core::FRAC_W);
        let texel = 2.0 / f64::from(size(l));
        let inverse = v.inverse;
        let mut anchor = |k: usize| {
            let w = [
                base[0].wrapping_add((k % 2) as i32 * ANCHOR),
                base[1].wrapping_add((k / 2) as i32 * ANCHOR),
            ];
            if let Some(&(_, at, frac)) = v.anchors.1.iter().find(|a| a.0 == w) {
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
            let (at, frac): (Vec<i32>, Vec<f32>) = (0..3)
                .map(|a| {
                    let v = p(a);
                    let r = v.round_ties_even();
                    ((r as i64) as i32, (v - r) as f32)
                })
                .unzip();
            let at = [at[0], at[1], at[2]];
            let frac = [frac[0], frac[1], frac[2]];
            v.anchors.1.push((w, at, frac));
            (at, frac)
        };
        let [a0, a1, a2, a3] = [0, 1, 2, 3].map(&mut anchor);
        // Only the anchors about the box are kept.
        v.anchors.1.retain(|a| {
            let d = [a.0[0].wrapping_sub(base[0]), a.0[1].wrapping_sub(base[1])];
            (0..=ANCHOR).contains(&d[0]) && (0..=ANCHOR).contains(&d[1])
        });
        let light = Light::of_words(&v.light_words);
        Starts {
            base,
            across: light.across.map(|v| v * q3 as f32),
            down: light.down.map(|v| v * q3 as f32),
            at: [a0.0, a1.0, a2.0, a3.0],
            frac: [a0.1, a1.1, a2.1, a3.1],
        }
    }

    /// Each level's frame, where the casters' depth pipelines read their
    /// view, and the box's corner and its layer the world's depth is laid in
    /// off.
    pub fn write(&self, g: &Gpu) {
        for (l, v) in self.levels.iter().enumerate().take(self.level_cnt as usize) {
            g.write_buf(v.light_frame, 0, bytes(&v.light));
            // Its layer of the world's array it lies in: the first's own, the
            // rest's one a level past the first.
            let layer = (l as u32).saturating_sub(1);
            let corner = [
                v.light_words[light::CORNER[0]],
                v.light_words[light::CORNER[1]],
                layer,
                0,
            ];
            g.write_buf(v.corner, 0, bytes(&corner));
        }
    }

    /// Level `l`'s pass, timed in the span `shadow`, the world's depth laid
    /// in first where `cast` (its cast holds this frame's world), one
    /// triangle over its layer; else cleared to the far plane, as nothing is
    /// cast there. In the claimed tiles of `tiles` alone where it is drawn
    /// through its gate (`gated`, its stencil numbered by `Tiles::gate`), a
    /// frame that casts, the rest kept as the last frame left it: the gate
    /// lays the world there and raises their stencil, a quad a tile of the
    /// box the frame's marks cover, the unclaimed collapsed, as many as the
    /// GPU counted (`Tiles::box_args`). The casters are drawn into it next,
    /// through the stencil, then it ends.
    pub fn begin<'a>(
        &mut self,
        ctx: &mut RenderContext<'a>,
        l: u32,
        cast: bool,
        tiles: &mut Tiles,
        gated: bool,
    ) -> TrackedRenderPass<'a> {
        let (g, encoder) = (ctx.g, ctx.encoder);
        let stamp = ctx.stamp(GpuSpanName::shadow);
        let load = if gated { Load::Load } else { Load::Clear };
        let v = &mut self.levels[l as usize];
        let desc = pass_desc(0, Load::Clear, v.map_view, load, 1.0, stamp.pass());
        let (desc, number) = tiles.gate(desc, gated);
        let pass = g.pass(encoder, &desc);
        let mut pass = TrackedRenderPass::new(g, pass);
        if let Some(n) = number {
            xpute_kit::ensure!(cast, EINVAL, n);
            let (words, uniform) = tiles.this_frame();
            let bound = (words, uniform, tiles.marks_box());
            if bound != v.gate_bound {
                release(v.gate_group);
                v.gate_group = g.bind_group(
                    self.gate.layouts[0],
                    &[
                        view_entry(0, v.world_array),
                        buf_entry(1, v.corner),
                        buf_entry(30, words),
                        buf_entry(31, uniform),
                        buf_entry(33, bound.2),
                    ],
                );
                v.gate_bound = bound;
            }
            pass.set_stencil_reference(n);
            pass.set_pipeline(self.gate.id);
            pass.set_bind_group(0, v.gate_group, &[]);
            if let Some(args) = tiles.box_args() {
                pass.draw_indirect(args, 0);
            }
        } else if cast {
            pass.set_pipeline(self.lay.id);
            pass.set_bind_group(0, v.lay_group, &[]);
            pass.draw(3, 1, 0, 0);
        }
        pass
    }
}
