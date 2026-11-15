// pxl-render/pass/atmosphere.rs

//! The atmosphere's uniform and its tables. Transmittance, multiple scattering and the sky's
//! irradiance are drawn once when the renderer starts. The sun and the moon each have a sky
//! view and a share of the air's volume, both laid out about the light's azimuth and the
//! world's up, so a turn of the eye keeps them. A light's two are drawn together where the
//! light's mu or the eye's altitude has moved half its step, and only while the light
//! reaches the air. The exposure is drawn where the words it reads moved or the ground under
//! the eye has moved half its step. A step is how far a quantity moves before what a table
//! shows moves a display level. The GPU measures each step while it draws the table, and
//! measures it again only where its quantity has left the sample the step was measured over.

use core::ops::Range;

use pxl_spec::bake::tile::earth;
use pxl_spec::space::coord::adapter::epsg3857::height_u_per_m;
use pxl_spec::space::coord::zoom::HEIGHT_BITS;
use pxl_spec::space::layer::demand::GROUND_DEMAND_TILE_CAP;
use pxl_spec::space::layer::lattice::TILE_CELLS;

use crate::frame::shader::record::{
    AIR, COMPUTE_ATMOS_EXPOSURE, COMPUTE_ATMOS_FROXEL, PIPELINE_ATMOS_IRRADIANCE,
    PIPELINE_ATMOS_MULTISCATTER, PIPELINE_ATMOS_SKY_VIEW, PIPELINE_ATMOS_TRANSMITTANCE,
};
use pxl_gpu_link::enums::{AddressMode, FilterMode, MipmapFilterMode};

use xpute_link::mem;

use crate::frame::shader::word::atmos::{
    CAMERA_ALT, DRAWN, EARTH_TILES, EARTH_WINDOW, KM_PER_U, LAND_ALT_KM, MOON_COLOR, MOON_DIR,
    MOON_LAYOUT, MOON_LIGHT, MOON_REACHES, SUN_DIR, SUN_LAYOUT, SUN_REACHES,
};
use crate::frame::shader::word::{
    AIR_FROXELS, ATMOS_FLOATS, EARTH_TILE_BITS, GROUND_SLOPES, IRRADIANCE_SIZE, LIGHT_SLOPES,
    MULTISCATTER_SIZE, SKY_VIEW_SIZE, SLOPE_WORDS, TRANSMITTANCE_SIZE,
};
use crate::gpu::api::{
    Entry, Format, Gpu, Layout, Load, RenderContext, buf_entry, buffer_usage, bytes, pass_desc,
    result, room, sampler_entry, signaled, texture_usage, view_entry, word_sz, write_texture_sz,
};
use crate::gpu::image_map::ImageMap;
use crate::gpu::pipeline::{Pipeline, PipelineCache};
use crate::gpu::read::Read;
use crate::look::atmosphere as model;
use crate::look::environment;
use crate::param::atmos::{ALT_SAMPLE, ALT_SAMPLE_LEAST_KM};
use crate::spec::span::GpuSpanName;

/// Hands each waiting earth tile's layer and texels to the writer, until the writer refuses
/// one by returning false.
pub type Upload = dyn FnMut(&mut dyn FnMut(u32, &[u8]) -> bool);

/// How many of the air's entries every painter and the sky read (record.rs `AIR`).
pub const AIR_ENTRY_CNT: usize = AIR.len();

/// One texture layer for each tile the earth layer's floor holds at once, which is one for
/// each slot of its tile cache.
pub const EARTH_LAYER_CNT: u32 = GROUND_DEMAND_TILE_CAP;
/// A tile's layer plus one takes `EARTH_TILE_BITS` bits, eight tiles to a word, in the four
/// words at `EARTH_TILES`.
const _: () = assert!(
    EARTH_LAYER_CNT < 1 << EARTH_TILE_BITS && EARTH_LAYER_CNT <= 32 / EARTH_TILE_BITS * 4,
    "the window's layers fit their word"
);

const TABLE_CNT: usize = 6;
/// The tables' sizes, in the order of their indices below: transmittance, multiple
/// scattering, the sun's sky view, the exposure texel, irradiance, and the moon's sky view.
/// The exposure's three texels hold its stops, then the sun's reach and the moon's
/// (atmos_exposure.wgsl).
pub(crate) const SIZES: [(u32, u32); TABLE_CNT] = [
    TRANSMITTANCE_SIZE,
    MULTISCATTER_SIZE,
    SKY_VIEW_SIZE,
    (3, 1),
    IRRADIANCE_SIZE,
    SKY_VIEW_SIZE,
];
pub(crate) const TRANSMITTANCE: usize = 0;
pub(crate) const MULTISCATTER: usize = 1;
pub(crate) const SKY_VIEW: usize = 2;
const EXPOSURE_TEXEL: usize = 3;
pub(crate) const IRRADIANCE: usize = 4;
const MOON_VIEW: usize = 5;

/// The share of its step that a quantity moves before its table is drawn again.
const STEP_SHARE: f32 = 0.5;
/// Half a display level. A light whose sky shows less than this along its vertical does not
/// reach the air, and its view and air are neither drawn nor read (atmos_exposure.wgsl
/// `reach`).
const REACH: f32 = 0.5;

/// What the views and the exposure measure, in words (atmos_skyview.wgsl `slopes`). Each
/// light has `LIGHT_SLOPES` words, the sun's first: the slope in its mu, the slope in the
/// eye's altitude, the mu's sample, the most display levels its sky shows, which says how
/// far it reaches, and that reach's slope in its mu. The ground's slope and sample
/// follow.
const SLOPES_SZ: u32 = 4 * SLOPE_WORDS;
const LIGHT_WORDS: usize = LIGHT_SLOPES as usize;
const GROUND_WORDS: usize = GROUND_SLOPES as usize;

/// A quantity whose slope is measured. Each has its bit in the masks of the measures due and
/// of those on their way (`bit`).
#[derive(Clone, Copy)]
enum Measure {
    Mu(usize),
    Alt(usize),
    Reach(usize),
    Ground,
}

impl Measure {
    const fn bit(self) -> u32 {
        match self {
            Measure::Mu(k) => 1 << (3 * k),
            Measure::Alt(k) => 1 << (3 * k + 1),
            Measure::Reach(k) => 1 << (3 * k + 2),
            Measure::Ground => 1 << 6,
        }
    }
}

/// The words the exposure texel is a function of (atmos_light.wgsl `eye_stops`), besides the
/// ground under the eye, at which it is held by a step (`Atmosphere::ground`): the lights'
/// directions, the moon's light and its color.
const EXPOSURE_READS: [Range<usize>; 2] = [SUN_DIR..CAMERA_ALT, MOON_COLOR..KM_PER_U];

/// Where each light's layout words and its reach flag lie (atmos_medium.wgsl `view_layout`,
/// `reaches`), the sun's first.
const LAYOUTS: [usize; 2] = [SUN_LAYOUT, MOON_LAYOUT];
const REACHES: [usize; 2] = [SUN_REACHES, MOON_REACHES];

/// A quantity at which a table is held until it moves a share of its measured step: a
/// light's mu, the eye's altitude, or the ground's elevation under the eye.
#[derive(Clone, Copy, Default)]
struct TableInput {
    /// The value the table was last held at.
    at: Option<f32>,
    /// How far the quantity moves before what the table shows moves a display level, by the
    /// last slope that landed; none before one has.
    step: Option<f32>,
    /// Where its slope was last measured, and the sample the slope was taken over.
    measured: Option<f32>,
    sample: f32,
}

impl TableInput {
    /// Whether `now` has moved `STEP_SHARE` of the step since the table was held. Before a
    /// step has landed, any move counts.
    fn changed(&self, now: f32) -> bool {
        match (self.at, self.step) {
            (None, _) => true,
            (Some(at), None) => at != now,
            (Some(at), Some(step)) => (now - at).abs() >= step * STEP_SHARE,
        }
    }

    /// Whether `now` has left the sample the slope was taken over, or no slope was measured
    /// yet.
    fn left(&self, now: f32) -> bool {
        self.measured
            .is_none_or(|at| (now - at).abs() >= self.sample)
    }
}

/// What is kept of a light's sky view and its half of the air's volume: the light's mu
/// and the eye's altitude they are held at, whether the light reaches the air, and whether
/// they were drawn where they were last held.
#[derive(Clone, Copy, Default)]
struct LightMemo {
    mu: TableInput,
    alt: TableInput,
    /// Whether the light reaches the air by `REACH`, as the exposure last measured; none
    /// before. While it does not, `to_reach` is how far its mu moves before it does, at
    /// the slope its reach last moved at (atmos_exposure.wgsl `reach`).
    reach: Option<bool>,
    to_reach: f32,
    /// Whether its view and air were drawn where they were last held. They were not where the
    /// light moved while it reached the air too little to be drawn.
    drawn: bool,
}

impl LightMemo {
    /// How far the light's mu moves before what its tables show moves a display level.
    /// In reach, that is its sky's step; out of reach, it is how far the mu moves before
    /// the light comes into reach. It is none before either has landed.
    fn step(&self) -> Option<f32> {
        match self.reach {
            Some(false) => Some(self.to_reach),
            _ => self.mu.step,
        }
    }
}

/// What a frame draws of the tables, as `prepare` decides it.
#[derive(Clone, Copy, Default)]
struct Plan {
    lights: [bool; 2],
    exposure: bool,
}

pub struct Atmosphere {
    uniform: u32,
    words: [f32; ATMOS_FLOATS],
    sampler: u32,
    views: [u32; TABLE_CNT],
    /// The tables' render pipelines (`pass_of`), and the exposure's compute pipeline. The
    /// exposure is a compute pass so that its span times its own work alone
    /// (`dispatch_exposure`).
    passes: [Pipeline; 4],
    exposure: Pipeline,
    groups: [u32; TABLE_CNT],
    baked: bool,
    /// Each light's own words, the sun's first (atmos_skyview.wgsl `light`): which light it
    /// is, and whether its drawing measures the slopes in its mu and in the eye's
    /// altitude. `exposure_words` are the exposure's own words, which say whether it measures
    /// the ground's slope (atmos_exposure.wgsl `own`).
    lights: [u32; 2],
    exposure_words: u32,
    /// What is kept of each light's view and air, the sun's first, and the ground the exposure
    /// is held at.
    light_memos: [LightMemo; 2],
    ground: TableInput,
    /// The buffer the views and the exposure write their measures into (`SLOPE_WORDS`), and the
    /// buffer the measures are mapped back through.
    slopes: (u32, u32),
    /// The measures that are due and not yet copied to be mapped, and those copied and on
    /// their way back, a bit each (`Measure::bit`).
    pending: u32,
    flying: u32,
    /// The measures as they land, and the mapping's fence and result. Both are on the heap,
    /// as the host writes them and a native build's statics lie outside the memory it
    /// writes. `ticket` is the core's number the mapping was last asked with, and `mapped`
    /// says whether a mapping is out, or landed and not yet taken.
    landed_words: Box<[u32; SLOPE_WORDS as usize]>,
    mapping: Box<[u32; 2]>,
    ticket: u32,
    mapped: bool,
    plan: Plan,
    sun_mu: f32,
    moon_mu: f32,
    /// The words the exposure texel was last drawn from (`EXPOSURE_READS`).
    exposure_read: Read,
    /// This month's picture of the Earth, zero until its image has come (`ImageMap::step`).
    /// Until then the land past the terrain shows only the air before it.
    pub earth: ImageMap,
    earth_tiles: (u32, u32),
    /// The air before each point (atmos_froxel.wgsl): the sun's half and the moon's, each
    /// drawn with its light's view from the same reads, per unit of its light and unexposed.
    /// `froxel` is the pipeline that makes it, and `froxel_groups` holds its group for each
    /// light.
    volume: u32,
    froxel: Pipeline,
    froxel_groups: [u32; 2],
}

impl Atmosphere {
    pub fn new(g: &Gpu, pipelines: &mut PipelineCache) -> Atmosphere {
        let uniform = g.buf(
            word_sz(ATMOS_FLOATS),
            buffer_usage::UNIFORM | buffer_usage::COPY_DST,
        );
        let mut words = [0.0f32; ATMOS_FLOATS];
        words[SUN_DIR + 2] = 1.0;

        let sampler = g.sampler(
            FilterMode::Linear,
            FilterMode::Linear,
            MipmapFilterMode::Nearest,
            AddressMode::ClampToEdge,
            None,
            1,
        );
        let mut textures = [0; TABLE_CNT];
        let mut views = [0; TABLE_CNT];
        for k in 0..TABLE_CNT {
            let (w, h) = SIZES[k];
            let written = if k == EXPOSURE_TEXEL {
                texture_usage::STORAGE_BINDING
            } else {
                texture_usage::RENDER_ATTACHMENT
            };
            textures[k] = g.texture(
                w,
                h,
                1,
                Format::Rgba16float,
                1,
                texture_usage::TEXTURE_BINDING | written,
            );
            views[k] = g.view(textures[k], false, 0, 1, 0, 1);
        }
        let passes = [
            PIPELINE_ATMOS_TRANSMITTANCE,
            PIPELINE_ATMOS_MULTISCATTER,
            PIPELINE_ATMOS_SKY_VIEW,
            PIPELINE_ATMOS_IRRADIANCE,
        ]
        .map(|id| pipelines.build(g, id, Format::Rgba16float));
        // The volume holds the sun's half across, then the moon's.
        let n = AIR_FROXELS;
        let volume = g.volume_view(g.volume(
            [2 * n, n, n],
            Format::Rgba16float,
            texture_usage::STORAGE_BINDING | texture_usage::TEXTURE_BINDING,
        ));

        let mut atmosphere = Atmosphere {
            uniform,
            words,
            sampler,
            views,
            passes,
            exposure: pipelines.build_compute(g, COMPUTE_ATMOS_EXPOSURE),
            groups: [0; TABLE_CNT],
            baked: false,
            lights: [0.0f32, 1.0].map(|which| {
                let light = g.buf(16, buffer_usage::UNIFORM | buffer_usage::COPY_DST);
                g.write_buf(light, 0, bytes(&[which, 0.0, 0.0, 0.0]));
                light
            }),
            exposure_words: {
                let own = g.buf(16, buffer_usage::UNIFORM | buffer_usage::COPY_DST);
                g.write_buf(own, 0, bytes(&[0.0f32; 4]));
                own
            },
            light_memos: [LightMemo::default(); 2],
            ground: TableInput::default(),
            slopes: (
                g.buf(
                    SLOPES_SZ,
                    buffer_usage::STORAGE | buffer_usage::COPY_SRC | buffer_usage::COPY_DST,
                ),
                g.buf(SLOPES_SZ, buffer_usage::MAP_READ | buffer_usage::COPY_DST),
            ),
            pending: 0,
            flying: 0,
            landed_words: Box::new([0; SLOPE_WORDS as usize]),
            mapping: Box::new([0; 2]),
            ticket: 0,
            mapped: false,
            plan: Plan::default(),
            exposure_read: Read::default(),
            sun_mu: 1.0,
            moon_mu: 1.0,
            earth: ImageMap::new(g, earth::PICTURE.0, earth::PICTURE.1),
            earth_tiles: {
                let texture = g.texture(
                    TILE_CELLS,
                    TILE_CELLS,
                    EARTH_LAYER_CNT,
                    Format::Rgba8unormSrgb,
                    1,
                    texture_usage::TEXTURE_BINDING | texture_usage::COPY_DST,
                );
                let view = g.view(texture, true, 0, 1, 0, EARTH_LAYER_CNT);
                (texture, view)
            },
            volume,
            froxel: pipelines.build_compute(g, COMPUTE_ATMOS_FROXEL),
            froxel_groups: [0; 2],
        };
        // Every table's group holds the air's entries its layout binds, then its own entries.
        let air = atmosphere.entries();
        let of = |layout: Layout, own: &[Entry]| -> Vec<Entry> {
            let shared = air.iter().filter(|e| layout.binds(e.binding));
            shared.chain(own).copied().collect()
        };
        let multiscatter = view_entry(12, views[MULTISCATTER]);
        let lights = atmosphere.lights;
        let light = |k: usize| buf_entry(14, lights[k]);
        // The exposure's layout binds the air's entries but two: the texel it writes, and the
        // volume.
        let own: [(usize, Vec<Entry>); TABLE_CNT] = [
            (TRANSMITTANCE, vec![]),
            (MULTISCATTER, vec![]),
            (IRRADIANCE, vec![multiscatter]),
            // The views write their measures too, and the slope in a light's mu counts the
            // land's lit face under the eye.
            (
                SKY_VIEW,
                vec![multiscatter, light(0), buf_entry(20, atmosphere.slopes.0)],
            ),
            (
                MOON_VIEW,
                vec![multiscatter, light(1), buf_entry(20, atmosphere.slopes.0)],
            ),
            (
                EXPOSURE_TEXEL,
                vec![
                    multiscatter,
                    view_entry(1, views[EXPOSURE_TEXEL]),
                    buf_entry(14, atmosphere.exposure_words),
                    buf_entry(20, atmosphere.slopes.0),
                ],
            ),
        ];
        for (k, own) in own {
            let layout = if k == EXPOSURE_TEXEL {
                atmosphere.exposure.layouts[0]
            } else {
                atmosphere.passes[pass_of(k)].layouts[0]
            };
            atmosphere.groups[k] = g.bind_group(layout, &of(layout, &own));
        }
        let froxel = atmosphere.froxel.layouts[0];
        for (k, group) in atmosphere.froxel_groups.iter_mut().enumerate() {
            let own = [multiscatter, view_entry(1, volume), light(k)];
            *group = g.bind_group(froxel, &of(froxel, &own));
        }
        atmosphere
    }

    /// The air's entries that every painter and the sky read (record.rs `AIR`).
    pub fn entries(&self) -> [Entry; AIR_ENTRY_CNT] {
        [
            buf_entry(8, self.uniform),
            view_entry(9, self.views[TRANSMITTANCE]),
            sampler_entry(11, self.sampler),
            view_entry(13, self.views[EXPOSURE_TEXEL]),
            view_entry(18, self.views[IRRADIANCE]),
            view_entry(22, self.volume),
        ]
    }

    /// The air's entries that the sky alone reads (record.rs `SKY_AIR`).
    pub fn sky_entries(&self) -> [Entry; 4] {
        [
            view_entry(10, self.views[SKY_VIEW]),
            view_entry(15, self.views[MOON_VIEW]),
            view_entry(16, self.earth.view),
            view_entry(17, self.earth_tiles.1),
        ]
    }

    /// Makes a light's share of the air before each point, the sun's (`k` 0) or the moon's,
    /// from the light and the eye's altitude now. `draw` makes it together with the light's
    /// view.
    fn froxel(&self, ctx: &mut RenderContext, k: usize) {
        let (g, encoder) = (ctx.g, ctx.encoder);
        let stamp = ctx.stamp(GpuSpanName::Air);
        let pass = g.compute_pass(encoder, stamp.pass());
        g.set_pipeline(pass, self.froxel.id);
        g.set_bind_group(pass, 0, self.froxel_groups[k], &[]);
        // A workgroup makes a column of froxels (atmos_froxel.wgsl).
        g.dispatch(pass, AIR_FROXELS, AIR_FROXELS, 1);
        g.end(pass);
    }

    /// Draws the exposure texel and how far each light reaches the air, a texel a thread
    /// (atmos_exposure.wgsl). It is a compute pass because a render pass's begin is stamped
    /// before the passes it waits on have finished, so a render pass's span would take in
    /// their time.
    fn dispatch_exposure(&self, ctx: &mut RenderContext) {
        let (g, encoder) = (ctx.g, ctx.encoder);
        let stamp = ctx.stamp(GpuSpanName::Exposure);
        let pass = g.compute_pass(encoder, stamp.pass());
        g.set_pipeline(pass, self.exposure.id);
        g.set_bind_group(pass, 0, self.groups[EXPOSURE_TEXEL], &[]);
        g.dispatch(pass, 1, 1, 1);
        g.end(pass);
    }

    /// Writes each earth tile that has come into its layer while the stream has room, and
    /// lays out in the words the window and which layer each tile reads. Returns whether a
    /// tile came this frame.
    pub fn set_earth_tiles(
        &mut self,
        g: &Gpu,
        window: [i32; 2],
        layers: &[u32],
        upload: &mut Upload,
    ) -> bool {
        let (texture, mut came) = (self.earth_tiles.0, false);
        upload(&mut |layer, texels| {
            if !room(write_texture_sz(texels)) {
                return false;
            }
            g.write_texture(texture, layer, TILE_CELLS, TILE_CELLS, texels);
            came = true;
            true
        });
        let w = &mut self.words;
        w[EARTH_WINDOW] = f32::from_bits(window[0] as u32);
        w[EARTH_WINDOW + 1] = f32::from_bits(window[1] as u32);
        let per_word = (32 / EARTH_TILE_BITS) as usize;
        let mut packed = [0u32; 4];
        for (k, &layer) in layers.iter().enumerate() {
            packed[k / per_word] |= layer << (EARTH_TILE_BITS * (k % per_word) as u32);
        }
        for (w, p) in w[EARTH_TILES..EARTH_TILES + 4].iter_mut().zip(packed) {
            *w = f32::from_bits(p);
        }
        came
    }

    /// The words the sky and the world read, as `prepare` wrote them.
    pub fn words(&self) -> &[f32] {
        &self.words
    }

    /// Lays out the words from the environment `env` and the rays' far end `far` and plans
    /// what this frame draws. Every light's and ray's mu is measured up the world's z,
    /// which is straight up where the eye stands.
    pub fn prepare(&mut self, g: &Gpu, env: &[f32], far: f32) {
        let (sun, moon) = (
            &env[environment::SUN..environment::SUN + 3],
            &env[environment::MOON..environment::MOON + 4],
        );
        let alt_km = env[environment::AIR];
        self.words[SUN_DIR..SUN_DIR + 3].copy_from_slice(sun);
        self.words[MOON_DIR..MOON_DIR + 3].copy_from_slice(&moon[..3]);
        self.words[MOON_LIGHT] = moon[3];
        self.words[MOON_COLOR..MOON_COLOR + 3]
            .copy_from_slice(&env[environment::MOON_COLOR..environment::MOON_COLOR + 3]);
        self.words[CAMERA_ALT] = alt_km;
        self.words[KM_PER_U] = env[environment::AIR + 1];
        self.words[LAND_ALT_KM] = env[environment::AIR + 2];
        self.words[DRAWN] = far;
        (self.sun_mu, self.moon_mu) = (sun[2], moon[2]);
        self.plan(g);
    }

    /// Writes `words` to the device. The frame calls it only when its read found them
    /// changed.
    pub fn write(&self, g: &Gpu) {
        g.write_buf(self.uniform, 0, bytes(&self.words));
    }

    /// Decides what this frame draws of the tables and what each drawing measures. It runs
    /// before the uniform is written, so the uniform holds each light's words as laid out. A
    /// light's view and air are drawn where its mu or the eye's altitude has moved half
    /// its step (`TableInput::changed`), or where the light has come to reach the air since
    /// they were last held. They are drawn only while the light reaches the air by `REACH`,
    /// and a reader reads them only then (atmos_medium.wgsl `reaches`). The exposure is drawn
    /// where the words it reads moved (`EXPOSURE_READS`), where the ground under the eye moved
    /// half its step, where a light's view or air was due, and in the frame that draws the
    /// tables drawn only once. Each time, it measures how far each light reaches. A slope is
    /// measured with its table's drawing where its quantity has left the sample it was taken
    /// over (`TableInput::left`), unless a measure of it is already on its way.
    fn plan(&mut self, g: &Gpu) {
        self.take_slopes();
        let fresh = !self.baked;
        if fresh {
            (self.light_memos, self.ground) = ([LightMemo::default(); 2], TableInput::default());
        }
        let busy = self.pending | self.flying;
        let alt = self.words[CAMERA_ALT];
        let mut due = false;
        for k in 0..2 {
            let mu = [self.sun_mu, self.moon_mu][k];
            let l = &mut self.light_memos[k];
            let input = TableInput {
                step: l.step(),
                ..l.mu
            };
            let changed = input.changed(mu) || l.alt.changed(alt);
            let reach = l.reach.unwrap_or(true);
            let draw = reach && (changed || !l.drawn);
            due |= changed;
            self.words[REACHES[k]] = f32::from(u8::from(reach));
            self.plan.lights[k] = draw;
            if changed || draw {
                (l.mu.at, l.alt.at, l.drawn) = (Some(mu), Some(alt), draw);
            }
            if !draw {
                continue;
            }
            let light = l.mu.left(mu) && busy & Measure::Mu(k).bit() == 0;
            let eye = l.alt.left(alt) && busy & Measure::Alt(k).bit() == 0;
            let base = (4 * LIGHT_WORDS * k) as u32;
            if light {
                l.mu.measured = Some(mu);
                g.write_buf(self.slopes.0, base, bytes(&[0u32]));
                g.write_buf(self.slopes.0, base + 8, bytes(&[0u32]));
                self.pending |= Measure::Mu(k).bit();
            }
            if eye {
                (l.alt.measured, l.alt.sample) = (Some(alt), alt_sample(alt));
                g.write_buf(self.slopes.0, base + 4, bytes(&[0u32]));
                self.pending |= Measure::Alt(k).bit();
            }
            let flag = |on: bool| f32::from(u8::from(on));
            g.write_buf(
                self.lights[k],
                0,
                bytes(&[k as f32, flag(light), flag(eye), 0.0]),
            );
            self.words[LAYOUTS[k]..LAYOUTS[k] + 3].copy_from_slice(&view_layout(alt));
        }
        let ground = self.words[LAND_ALT_KM];
        let e = &mut self.exposure_read;
        e.begin();
        for at in EXPOSURE_READS {
            e.read(&self.words[at]);
        }
        e.mark(fresh || due || self.ground.changed(ground));
        self.plan.exposure = !e.end();
        if self.plan.exposure {
            self.ground.at = Some(ground);
            let measure = self.ground.left(ground) && busy & Measure::Ground.bit() == 0;
            if measure {
                self.ground.measured = Some(ground);
                self.pending |= Measure::Ground.bit();
            }
            let flag = f32::from(u8::from(measure));
            g.write_buf(self.exposure_words, 0, bytes(&[0.0, flag, 0.0, 0.0]));
            self.pending |= Measure::Reach(0).bit() | Measure::Reach(1).bit();
        }
    }

    /// Draws what `plan` decided. It returns whether a table the world reads was drawn again,
    /// whether the exposure texel was, and whether the measures were copied to be mapped back.
    /// Measures are copied only once no earlier ones are on their way.
    pub fn draw(&mut self, ctx: &mut RenderContext) -> (bool, bool, bool) {
        let (g, encoder) = (ctx.g, ctx.encoder);
        let mut drew = !self.baked;
        if !self.baked {
            self.table(ctx, TRANSMITTANCE, GpuSpanName::Air);
            self.table(ctx, MULTISCATTER, GpuSpanName::Air);
            self.table(ctx, IRRADIANCE, GpuSpanName::Air);
            self.baked = true;
        }
        // Each light's froxels go before its table, which reads none of them. The span `air`
        // then begins on a compute pass, whose begin waits on what runs before it, as a render
        // pass's does not (gpu/api.rs `take_stamps`).
        for (k, table) in [SKY_VIEW, MOON_VIEW].into_iter().enumerate() {
            if self.plan.lights[k] {
                self.froxel(ctx, k);
                self.table(ctx, table, GpuSpanName::Air);
                drew = true;
            }
        }
        let exposed = self.plan.exposure;
        if exposed {
            self.dispatch_exposure(ctx);
        }
        let copied = self.pending != 0 && self.flying == 0;
        if copied {
            g.copy_buf(encoder, (self.slopes.0, 0), (self.slopes.1, 0), SLOPES_SZ);
            self.flying = core::mem::take(&mut self.pending);
        }
        (drew, exposed, copied)
    }

    /// Asks for the measures to be mapped back, once the encoder that copies them is
    /// submitted. Their landing wakes the frame that reads them.
    pub fn read_back(&mut self, g: &Gpu) {
        if self.flying != 0 && !self.mapped {
            let addr = mem::addr(&*self.landed_words);
            self.ticket = self.ticket.wrapping_add(1);
            g.map_read(
                self.slopes.1,
                0,
                SLOPES_SZ,
                addr,
                &self.mapping,
                self.ticket,
            );
            self.mapped = true;
        }
    }

    /// Whether measures have landed for a frame to take (`take_slopes`). A refused mapping
    /// asks for no frame; what it carried is copied again with the next frame drawn.
    pub fn complete(&self) -> bool {
        // SAFETY: the mapping's fence, boxed in place; the host stores into it only atomically.
        self.mapped
            && unsafe { signaled(core::ptr::addr_of!(self.mapping[0]), self.ticket) }
            && self.mapping[1] == result::OK
    }

    /// Takes the measures that landed. Each slope becomes a step: how far its quantity moves
    /// while what its table shows moves one display level at that slope. The sample the slope
    /// was taken over is kept with it, and the reach measures say whether each light reaches
    /// the air.
    fn take_slopes(&mut self) {
        // SAFETY: the mapping's fence, boxed in place; the host stores into it only atomically.
        if !self.mapped || !unsafe { signaled(core::ptr::addr_of!(self.mapping[0]), self.ticket) } {
            return;
        }
        self.mapped = false;
        // A refused mapping measured nothing, so what it carried is measured again.
        if self.mapping[1] != result::OK {
            self.pending |= core::mem::take(&mut self.flying);
            return;
        }
        let (w, flying) = (*self.landed_words, self.flying);
        let word = |at: usize| f32::from_bits(w[at]);
        let has = |m: Measure| flying & m.bit() != 0;
        for (k, l) in self.light_memos.iter_mut().enumerate() {
            let base = LIGHT_WORDS * k;
            if has(Measure::Mu(k)) {
                (l.mu.step, l.mu.sample) = (Some(1.0 / word(base)), word(base + 2));
            }
            if has(Measure::Alt(k)) {
                l.alt.step = Some(1.0 / word(base + 1));
            }
            if has(Measure::Reach(k)) {
                let shows = word(base + 3);
                l.reach = Some(shows >= REACH);
                l.to_reach = (REACH - shows).max(0.0) / word(base + 4);
            }
        }
        if has(Measure::Ground) {
            self.ground.step = Some(1.0 / word(GROUND_WORDS));
            self.ground.sample = word(GROUND_WORDS + 1);
        }
        self.flying = 0;
    }

    /// Each light's step (`LightMemo::step`), the sun's first. In reach, it is the mu over
    /// which the light's sky moves a display level; out of reach, it is how far the mu
    /// moves before the light comes into reach. It is 0, the least, before either has landed.
    pub fn steps(&self) -> [f32; 2] {
        self.light_memos.map(|l| l.step().unwrap_or(0.0))
    }

    fn table(&self, ctx: &mut RenderContext, k: usize, span: GpuSpanName) {
        let (g, encoder) = (ctx.g, ctx.encoder);
        let stamp = ctx.stamp(span);
        let pass = g.pass(
            encoder,
            &pass_desc(
                self.views[k],
                Load::Clear,
                0,
                Load::Clear,
                0.0,
                stamp.pass(),
            ),
        );
        g.set_pipeline(pass, self.passes[pass_of(k)].id);
        g.set_bind_group(pass, 0, self.groups[k], &[]);
        g.draw(pass, 3, 1, 0, 0);
        g.end(pass);
    }
}

/// A table's render pipeline among `passes`. The moon's sky view is drawn by the sky view's
/// pipeline, and the exposure has its own compute pipeline apart.
fn pass_of(table: usize) -> usize {
    match table {
        TRANSMITTANCE => 0,
        MULTISCATTER => 1,
        SKY_VIEW | MOON_VIEW => 2,
        IRRADIANCE => 3,
        _ => xpute_kit::bug!(EINVAL, table),
    }
}

/// The sample of the eye's altitude `alt` that its slope is taken over: `ALT_SAMPLE` of the
/// altitude, and at least `ALT_SAMPLE_LEAST_KM` (atmos_skyview.wgsl `alt_sample`).
fn alt_sample(alt: f32) -> f32 {
    (alt * ALT_SAMPLE).max(ALT_SAMPLE_LEAST_KM)
}

/// A light's tables as laid out with the eye at `alt` km (atmos_medium.wgsl `view_layout`):
/// the angle from straight down to the horizon there, and the near and far ends of the air's
/// depth law, in km. The near end is how far the eye stands above the world's top, and 0 while
/// the eye stands within the world. The world's top is the greatest Z a position holds,
/// `HEIGHT_BITS` bits about sea level, taken in meters where a unit is longest, at the
/// equator (pxl-spec 3857.rs `height_u_per_m`). So an eye high over the world spends no
/// slice on the empty air under it, whatever latitude a unit is measured at. The far end is
/// how far a ray from the eye runs, past the horizon, to where it leaves the air.
fn view_layout(alt: f32) -> [f32; 3] {
    let (b, t) = (model::BOTTOM_RADIUS_KM, model::TOP_RADIUS_KM);
    let alt = f64::from(alt);
    let r = b + alt.max(0.001);
    let across = libm::sqrt((r * r - b * b).max(0.0));
    let top = f64::from(1u32 << (HEIGHT_BITS - 1)) / height_u_per_m(0.0) / 1000.0;
    let near = (alt - top).max(0.0);
    let far = (across + libm::sqrt(t * t - b * b)).max(near + 0.001);
    [
        libm::acos((across / r).clamp(-1.0, 1.0)) as f32,
        near as f32,
        far as f32,
    ]
}
