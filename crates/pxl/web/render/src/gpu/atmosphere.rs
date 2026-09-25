// pxl-render/gpu/atmosphere.rs

//! The atmosphere on the device: the uniform its numbers are read through
//! (look/atmosphere.rs's, with the frame's sun and camera), and the three
//! tables Hillaire's technique draws the sky from, each a full-screen pass
//! into a texture of four half floats.
//!
//! - The transmittance, 256 x 64: what light survives from the top of the
//!   atmosphere to each altitude along each direction. Once for a device.
//! - The multiple scattering, 32 x 32: what light scattered more than once
//!   adds at each altitude under each height of the sun. Once, after it.
//! - The sky view, 192 x 108: the sky in every direction from the camera's
//!   altitude with the sun at its height. Drawn again only when either has
//!   moved: the sun crawls, and the camera's altitude changes while it
//!   zooms and not while it pans or turns, since the table is kept relative
//!   to the sun's azimuth.
//! - The exposure, 2 x 1: what the eye has adapted the air's light to, from
//!   the sky view and the sun, with the zenith's sky beside it; and the
//!   sky's irradiance on a face turned up, the terrain's fill
//!   (atmos_exposure.wgsl). Drawn after the sky view, whenever that is;
//!   nothing reads it back.
//! - The moonlit sky, 192 x 108: the sky view again, lit by the moon and
//!   kept relative to its azimuth. Drawn only while the moon lights a sky
//!   the frame shows, and again when its height or the camera's altitude
//!   has moved.
//!
//! What it costs: the two fixed tables once, about two million samples of
//! the air between them; the sky view's 20,736 texels of 30 steps each when
//! it moves; and about 300 KB of textures, eight bytes a texel. The sky and
//! every lit fragment then read the tables instead of marching.

use pxl_spec::space::v3::V3;

use crate::frame::shader::record::{PIPELINE_ATMOS_EXPOSURE, PIPELINE_ATMOS_MULTISCATTER, PIPELINE_ATMOS_SKY_VIEW, PIPELINE_ATMOS_TRANSMITTANCE};
use crate::gpu::api::{buffer_entry, buffer_usage, bytes, object, pass_desc, release, texture_usage, word_bytes, GpuApi, Stamp};
use crate::gpu::api_spec::{address, compare, dimension, filter, format, load, resource};
use crate::gpu::clouds::CloudField;
use crate::gpu::pipeline::{Built, Pipelines};
use crate::look::atmosphere as model;
use crate::look::environment::{ENV_AIR, ENV_MOON, ENV_SUN};

/// Floats the atmosphere uniform (atmos_medium.wgsl's Atmosphere) holds.
/// Two past the last field: the struct is laid out in 16-byte rows and the
/// device rounds it up to one (record.test.rs holds this to naga).
pub const ATMOS_FLOATS: usize = 56;
/// Where each field sits in it, in floats (record.test.rs holds them to the
/// struct). The frame's own are written every frame; the rest are the
/// model's, laid out once.
pub(crate) const RAYLEIGH_SCATTERING: usize = 0;
pub(crate) const BOTTOM_RADIUS: usize = 3;
pub(crate) const OZONE_ABSORPTION: usize = 4;
pub(crate) const TOP_RADIUS: usize = 7;
pub(crate) const WHITE: usize = 8;
pub(crate) const RAYLEIGH_SCALE_HEIGHT: usize = 11;
pub(crate) const SUN_DIR: usize = 12;
pub(crate) const MIE_SCATTERING: usize = 15;
pub(crate) const CAMERA_ALTITUDE: usize = 16;
pub(crate) const KM_PER_U: usize = 17;
pub(crate) const MIE_EXTINCTION: usize = 18;
pub(crate) const MIE_SCALE_HEIGHT: usize = 19;
pub(crate) const MIE_G: usize = 20;
pub(crate) const OZONE_CENTER: usize = 21;
pub(crate) const OZONE_HALF_WIDTH: usize = 22;
pub(crate) const GROUND_ALBEDO: usize = 23;
pub(crate) const SUN_ANGULAR_RADIUS: usize = 24;
pub(crate) const EXPOSURE: usize = 25;
pub(crate) const MAX_ADAPTATION: usize = 26;
pub(crate) const ADAPTATION: usize = 27;
pub(crate) const MOON_DIR: usize = 28;
pub(crate) const MOON_LIGHT: usize = 31;
pub(crate) const LAND: usize = 32;
pub(crate) const LAND_ALTITUDE: usize = 35;
pub(crate) const SUN_SEEN: usize = 36;
pub(crate) const UP: usize = 40;
/// The cloud field (gpu/clouds.rs's `field`), in its order: the lattice
/// index and fraction, the clock, the count, the layer's height and where
/// it is gone.
pub(crate) const CLOUD_BASE: usize = 44;
pub(crate) const CLOUD_FRAC: usize = 46;
pub(crate) const CLOUD_TIME: usize = 48;
pub(crate) const CLOUD_COUNT: usize = 49;
pub(crate) const CLOUD_ALTITUDE: usize = 50;
pub(crate) const CLOUD_GONE: usize = 51;
/// The surface haze (look/atmosphere.rs), read by the aerial perspective.
pub(crate) const HAZE_EXTINCTION: usize = 52;
pub(crate) const HAZE_SCALE_HEIGHT: usize = 53;

/// The air's entries in a pass's group, 8 through 15, six of five words.
pub const AIR_WORDS: usize = 30;

/// The tables' sizes, as atmos_medium.wgsl's *_SIZE constants (record.test.rs
/// holds them to it), the exposure's texel, and the moonlit sky's, the sky
/// view's.
pub(crate) const SIZES: [(u32, u32); 5] = [(256, 64), (32, 32), (192, 108), (2, 1), (192, 108)];
pub(crate) const TRANSMITTANCE: usize = 0;
pub(crate) const MULTISCATTER: usize = 1;
pub(crate) const SKY_VIEW: usize = 2;
const EXPOSURE_TEXEL: usize = 3;
const MOON_VIEW: usize = 4;

/// How far the sun's height, as a cosine, moves before the sky is drawn
/// again: a thousandth of a radian or so near the horizon, where the colours
/// move fastest.
const SUN_MU_STEP: f32 = 2e-4;
/// How far the camera's altitude moves before the sky is drawn again, as a
/// share of it, and at least a metre.
const ALTITUDE_SHARE: f32 = 0.005;
const ALTITUDE_FLOOR_KM: f32 = 0.001;

pub struct Atmosphere {
    uniform: u32,
    words: [f32; ATMOS_FLOATS],
    sampler: u32,
    views: [u32; 5],
    passes: [Built; 4],
    groups: [u32; 5],
    /// Whether the two fixed tables are drawn on this device.
    baked: bool,
    /// The sun's cosine and the camera's altitude the sky view was drawn
    /// for; none before the first.
    drawn_for: Option<(f32, f32)>,
    /// The moon's cosine and the camera's altitude the moonlit sky was
    /// drawn for.
    moon_drawn_for: Option<(f32, f32)>,
    /// The sun's and the moon's cosines from the zenith at the eye this frame.
    sun_mu: f32,
    moon_mu: f32,
}

impl Atmosphere {
    pub fn new(g: &GpuApi, pipelines: &mut Pipelines) -> Atmosphere {
        let uniform = object();
        (g.buffer)(uniform, word_bytes(ATMOS_FLOATS), buffer_usage::UNIFORM | buffer_usage::COPY_DST);
        let white = model::white();
        let mut words = [0.0f32; ATMOS_FLOATS];
        for (at, rgb) in [
            (RAYLEIGH_SCATTERING, model::RAYLEIGH_SCATTERING),
            (OZONE_ABSORPTION, model::OZONE_ABSORPTION),
            (WHITE, white),
            (LAND, model::land()),
        ] {
            for (w, v) in words[at..at + 3].iter_mut().zip(rgb) {
                *w = v as f32;
            }
        }
        for (at, v) in [
            (BOTTOM_RADIUS, model::BOTTOM_RADIUS_KM),
            (TOP_RADIUS, model::TOP_RADIUS_KM),
            (RAYLEIGH_SCALE_HEIGHT, model::RAYLEIGH_SCALE_HEIGHT_KM),
            (MIE_SCATTERING, model::MIE_SCATTERING),
            (MIE_EXTINCTION, model::MIE_EXTINCTION),
            (HAZE_EXTINCTION, model::HAZE_EXTINCTION),
            (HAZE_SCALE_HEIGHT, model::HAZE_SCALE_HEIGHT_KM),
            (MIE_SCALE_HEIGHT, model::MIE_SCALE_HEIGHT_KM),
            (MIE_G, model::MIE_G),
            (OZONE_CENTER, model::OZONE_CENTER_KM),
            (OZONE_HALF_WIDTH, model::OZONE_HALF_WIDTH_KM),
            (GROUND_ALBEDO, model::GROUND_ALBEDO),
            (SUN_ANGULAR_RADIUS, model::SUN_ANGULAR_RADIUS),
            (EXPOSURE, model::EXPOSURE),
            (MAX_ADAPTATION, model::MAX_ADAPTATION),
            (ADAPTATION, model::ADAPTATION),
        ] {
            words[at] = v as f32;
        }
        words[SUN_DIR + 2] = 1.0;

        let sampler = object();
        (g.sampler)(sampler, filter::LINEAR, filter::LINEAR, filter::NEAREST, address::CLAMP, compare::NONE, 1);
        let mut textures = [0; 5];
        let mut views = [0; 5];
        for k in 0..5 {
            textures[k] = object();
            let (w, h) = SIZES[k];
            (g.texture)(textures[k], w, h, 1, format::RGBA16F, 1, texture_usage::TEXTURE_BINDING | texture_usage::RENDER_ATTACHMENT);
            views[k] = object();
            (g.view)(views[k], textures[k], dimension::D2, 0, 1, 0, 1);
        }
        let passes = [PIPELINE_ATMOS_TRANSMITTANCE, PIPELINE_ATMOS_MULTISCATTER, PIPELINE_ATMOS_SKY_VIEW, PIPELINE_ATMOS_EXPOSURE].map(|id| pipelines.build(g, id, format::RGBA16F));

        let air = buffer_entry(8, uniform);
        let transmittance = [9, resource::VIEW, views[TRANSMITTANCE], 0, 0];
        let linear = [11, resource::SAMPLER, sampler, 0, 0];
        let multiscatter = [12, resource::VIEW, views[MULTISCATTER], 0, 0];
        let sky_view = [10, resource::VIEW, views[SKY_VIEW], 0, 0];
        let mut groups = [0; 5];
        let mut e = [0u32; 25];
        e[..5].copy_from_slice(&air);
        e[5..10].copy_from_slice(&transmittance);
        e[10..15].copy_from_slice(&linear);
        e[15..20].copy_from_slice(&multiscatter);
        for (k, n) in [(TRANSMITTANCE, 5), (MULTISCATTER, 15)] {
            groups[k] = object();
            (g.bind_group)(groups[k], passes[k].layouts[0], &e[..n]);
        }
        // The sky view's two groups, told which light by a word of their own.
        for (k, which) in [(SKY_VIEW, 0.0f32), (MOON_VIEW, 1.0)] {
            let light = object();
            (g.buffer)(light, 16, buffer_usage::UNIFORM | buffer_usage::COPY_DST);
            (g.write_buffer)(light, 0, bytes(&[which, 0.0, 0.0, 0.0]));
            e[20..25].copy_from_slice(&buffer_entry(14, light));
            groups[k] = object();
            (g.bind_group)(groups[k], passes[SKY_VIEW].layouts[0], &e);
        }
        e[15..20].copy_from_slice(&sky_view);
        groups[EXPOSURE_TEXEL] = object();
        (g.bind_group)(groups[EXPOSURE_TEXEL], passes[EXPOSURE_TEXEL].layouts[0], &e[..20]);
        Atmosphere {
            uniform,
            words,
            sampler,
            views,
            passes,
            groups,
            baked: false,
            drawn_for: None,
            moon_drawn_for: None,
            sun_mu: 1.0,
            moon_mu: 1.0,
        }
    }

    /// The air as a pass's group entries, bindings 8 through 15: the
    /// uniform, the transmittance and sky-view tables, their sampler, the
    /// exposure's texel and the moonlit sky's table.
    pub fn entries(&self) -> [u32; AIR_WORDS] {
        let mut e = [0u32; AIR_WORDS];
        e[..5].copy_from_slice(&buffer_entry(8, self.uniform));
        e[5..10].copy_from_slice(&[9, resource::VIEW, self.views[TRANSMITTANCE], 0, 0]);
        e[10..15].copy_from_slice(&[10, resource::VIEW, self.views[SKY_VIEW], 0, 0]);
        e[15..20].copy_from_slice(&[11, resource::SAMPLER, self.sampler, 0, 0]);
        e[20..25].copy_from_slice(&[13, resource::VIEW, self.views[EXPOSURE_TEXEL], 0, 0]);
        e[25..30].copy_from_slice(&[15, resource::VIEW, self.views[MOON_VIEW], 0, 0]);
        e
    }

    /// The frame's air, from the environment's lane `env`: the sun's
    /// direction in the root frame, unturned; the moon's and how bright its
    /// sky is shown; the camera's altitude in kilometres, a unit's length in
    /// them and the ground's own height, which the sky draws its ground at;
    /// and straight up where the eye stands, which every height of a light or
    /// a ray is measured from.
    pub fn lay_out(&mut self, g: &GpuApi, env: &[f32], up: &V3) {
        let (sun, moon) = (&env[ENV_SUN..ENV_SUN + 3], &env[ENV_MOON..ENV_MOON + 4]);
        let altitude_km = env[ENV_AIR];
        self.words[SUN_DIR..SUN_DIR + 3].copy_from_slice(sun);
        self.words[MOON_DIR..MOON_DIR + 3].copy_from_slice(&moon[..3]);
        self.words[MOON_LIGHT] = moon[3];
        self.words[CAMERA_ALTITUDE] = altitude_km;
        self.words[KM_PER_U] = env[ENV_AIR + 1];
        self.words[LAND_ALTITUDE] = env[ENV_AIR + 3];
        for (w, v) in self.words[UP..UP + 3].iter_mut().zip(up) {
            *w = *v as f32;
        }
        let height = |d: &[f32]| (d[0] as f64 * up[0] + d[1] as f64 * up[1] + d[2] as f64 * up[2]) as f32;
        (self.sun_mu, self.moon_mu) = (height(sun), height(moon));
        for (w, v) in self.words[SUN_SEEN..SUN_SEEN + 3].iter_mut().zip(model::light_at(altitude_km as f64, self.sun_mu as f64)) {
            *w = v as f32;
        }
        (g.write_buffer)(self.uniform, 0, bytes(&self.words));
    }

    /// The cloud field for the frame (gpu/clouds.rs's `field`): written with
    /// the rest by `lay_out`, which the renderer calls after it.
    pub fn set_clouds(&mut self, field: &CloudField) {
        let w = &mut self.words;
        for k in 0..2 {
            w[CLOUD_BASE + k] = f32::from_bits(field.base[k]);
            w[CLOUD_FRAC + k] = field.frac[k];
        }
        w[CLOUD_TIME] = field.time;
        w[CLOUD_COUNT] = f32::from_bits(field.count);
        w[CLOUD_ALTITUDE] = field.altitude_km;
        w[CLOUD_GONE] = field.gone_km;
    }

    /// Draws what is due into `encoder`, ahead of every pass that reads the
    /// tables: the fixed two the first time, the sky view when the sun or the
    /// camera has moved far enough to show; timed under `stamp`.
    pub fn draw(&mut self, g: &GpuApi, encoder: u32, stamp: &mut Stamp) {
        if !self.baked {
            self.table(g, encoder, TRANSMITTANCE, stamp);
            self.table(g, encoder, MULTISCATTER, stamp);
            self.baked = true;
            self.drawn_for = None;
            self.moon_drawn_for = None;
        }
        let altitude = self.words[CAMERA_ALTITUDE];
        let now = (self.sun_mu, altitude);
        if moved(self.drawn_for, now) {
            self.table(g, encoder, SKY_VIEW, stamp);
            self.table(g, encoder, EXPOSURE_TEXEL, stamp);
            self.drawn_for = Some(now);
        }
        let moon = (self.moon_mu, altitude);
        if self.words[MOON_LIGHT] > 0.0 && moved(self.moon_drawn_for, moon) {
            self.table(g, encoder, MOON_VIEW, stamp);
            self.moon_drawn_for = Some(moon);
        }
    }

    fn table(&self, g: &GpuApi, encoder: u32, k: usize, stamp: &mut Stamp) {
        let pass = object();
        (g.pass)(pass, encoder, &pass_desc(self.views[k], load::CLEAR, 0, load::CLEAR, 0.0, stamp.pass()));
        (g.set_pipeline)(pass, self.passes[if k == MOON_VIEW { SKY_VIEW } else { k }].pipeline);
        (g.set_bind_group)(pass, 0, self.groups[k], &[]);
        (g.draw)(pass, 3, 1, 0, 0);
        (g.end)(pass);
        release(pass);
    }
}

/// Whether a table drawn for `drawn` — its light's cosine and the camera's
/// altitude — is due again at `now`.
fn moved(drawn: Option<(f32, f32)>, now: (f32, f32)) -> bool {
    match drawn {
        None => true,
        Some((mu, h)) => (now.0 - mu).abs() > SUN_MU_STEP || (now.1 - h).abs() > (h.abs() * ALTITUDE_SHARE).max(ALTITUDE_FLOOR_KM),
    }
}
