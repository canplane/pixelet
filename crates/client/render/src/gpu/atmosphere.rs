// pxl-render/gpu/atmosphere.rs

//! The atmosphere's uniform and its tables: transmittance, multiple scattering
//! and the sky's irradiance once per device; the sun's and moon's sky views
//! redrawn only when the light's height or the camera's altitude moves (the
//! table is kept relative to the light's azimuth); and every frame the exposure.

use pxl_spec::bake::tile::earth::{self, EARTH_ZOOM};
use pxl_spec::space::layer::demand::{REACH_ACROSS, REACH_TILES};
use pxl_spec::space::layer::lattice::TILE_CELLS;

use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world::around;

use crate::frame::shader::record::{PIPELINE_ATMOS_EXPOSURE, PIPELINE_ATMOS_IRRADIANCE, PIPELINE_ATMOS_MULTISCATTER, PIPELINE_ATMOS_SKY_VIEW, PIPELINE_ATMOS_TRANSMITTANCE};
use xpute_gpu::enums::{AddressMode, FilterMode, MipmapFilterMode};

use xpute_guest::mem::base::off_of;

use crate::gpu::api::{buffer_entry, buffer_usage, bytes, pass_desc, room, texture_usage, word_bytes, Format, Gpu, Load, Stamp, DONE};
use crate::gpu::api_spec::resource;
use crate::gpu::image_map::ImageMap;
use crate::gpu::layers::Mips;
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::renderer::Upload;
use crate::look::atmosphere as model;
use crate::look::environment::{ENV_AIR, ENV_MOON, ENV_SKY, ENV_SUN};

/// One past the last field: the device rounds the struct up to a 16-byte row.
pub const ATMOS_FLOATS: usize = 52;
/// Offsets in floats into the struct.
pub(crate) const RAYLEIGH_SCATTERING: usize = 0;
pub(crate) const BOTTOM_RADIUS: usize = 3;
pub(crate) const OZONE_ABSORPTION: usize = 4;
pub(crate) const TOP_RADIUS: usize = 7;
// 8 to 10 free.
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
pub(crate) const NIGHT_STOPS: usize = 26;
pub(crate) const SHOWN_FALL: usize = 27;
pub(crate) const MOON_DIR: usize = 28;
pub(crate) const MOON_LIGHT: usize = 31;
// 32 to 34 free.
pub(crate) const CENTER_LON: usize = 35;
pub(crate) const NIGHT_SKY: usize = 36;
pub(crate) const LAND_ALTITUDE: usize = 39;
pub(crate) const CITY_LIGHT: usize = 40;
pub(crate) const EARTH_REACH: usize = 43;
pub(crate) const EARTH_WINDOW: usize = 44;
pub(crate) const EARTH_TILES: usize = 48;

/// Bindings 8 through 19, ten entries of five words.
pub const AIR_WORDS: usize = 50;

/// A layer for each tile the earth layer's floor holds at once: its residency's frames.
pub const EARTH_LAYERS: u32 = REACH_TILES;
/// A tile's layer plus one in four bits, eight to a word.
pub(crate) const TILE_BITS: u32 = 4;
const _: () = assert!(EARTH_LAYERS < 1 << TILE_BITS && EARTH_LAYERS <= 32 / TILE_BITS * 4, "the window's layers fit their word");

/// The size bake_earth writes the Earth's global picture at.
const EARTH_SIZE: (u32, u32) = (2048, 1024);

const TABLES: usize = 6;
/// Transmittance, multiple scattering, sky view, exposure, irradiance, moonlit sky.
pub(crate) const SIZES: [(u32, u32); TABLES] = [(256, 64), (32, 32), (192, 108), (1, 1), (64, 16), (192, 108)];
pub(crate) const TRANSMITTANCE: usize = 0;
pub(crate) const MULTISCATTER: usize = 1;
pub(crate) const SKY_VIEW: usize = 2;
const EXPOSURE_TEXEL: usize = 3;
pub(crate) const IRRADIANCE: usize = 4;
const MOON_VIEW: usize = 5;

/// A share of the altitude, and at least a meter.
const ALTITUDE_SHARE: f32 = 0.005;
const ALTITUDE_FLOOR_KM: f32 = 0.001;

pub struct Atmosphere {
    uniform: u32,
    words: [f32; ATMOS_FLOATS],
    sampler: u32,
    views: [u32; TABLES],
    passes: [Built; 5],
    groups: [u32; TABLES],
    baked: bool,
    /// The light's height and the eye's altitude each view was drawn for,
    /// the sun's first.
    drawn_for: [Option<(f32, f32)>; 2],
    /// Each light's slope as its sky view was last drawn, in display levels
    /// per unit of its height (atmos_skyview.wgsl `slopes`), the sun's
    /// first; and the buffer they are mapped back through.
    slopes: (u32, u32),
    /// Which lights' views the slopes now on their way are of.
    slopes_of: [bool; 2],
    /// The slopes' bits as they land, and their word (`DONE` once in).
    landed: [u32; 2],
    landed_word: u32,
    /// The height each light moves before its sky changes by a display
    /// level, as the last landed slopes say; none before any landed.
    steps: [Option<f32>; 2],
    sun_mu: f32,
    moon_mu: f32,
    earth: ImageMap,
    earth_tiles: (u32, u32),
}

impl Atmosphere {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines, frame_buffer: u32) -> Atmosphere {
        let uniform = g.buffer(word_bytes(ATMOS_FLOATS), buffer_usage::UNIFORM | buffer_usage::COPY_DST);
        let mut words = [0.0f32; ATMOS_FLOATS];
        for (at, rgb) in [
            (RAYLEIGH_SCATTERING, model::RAYLEIGH_SCATTERING),
            (OZONE_ABSORPTION, model::OZONE_ABSORPTION),
            (NIGHT_SKY, core::array::from_fn(|k| model::NIGHT_COLOR[k] * model::NIGHT_SKY)),
            (CITY_LIGHT, core::array::from_fn(|k| model::CITY_COLOR[k] * model::CITY_LIGHT)),
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
            (MIE_SCALE_HEIGHT, model::MIE_SCALE_HEIGHT_KM),
            (MIE_G, model::MIE_G),
            (OZONE_CENTER, model::OZONE_CENTER_KM),
            (OZONE_HALF_WIDTH, model::OZONE_HALF_WIDTH_KM),
            (GROUND_ALBEDO, model::GROUND_ALBEDO),
            (SUN_ANGULAR_RADIUS, model::SUN_ANGULAR_RADIUS),
            (EXPOSURE, model::EXPOSURE),
            (NIGHT_STOPS, model::NIGHT_STOPS),
            (SHOWN_FALL, model::SHOWN_FALL),
        ] {
            words[at] = v as f32;
        }
        words[SUN_DIR + 2] = 1.0;
        words[EARTH_REACH] = earth::EARTH_REACH as f32;
        words[EARTH_WINDOW + 2] = f32::from_bits(REACH_ACROSS);
        words[EARTH_WINDOW + 3] = f32::from_bits(around(EARTH_ZOOM) as u32);

        let sampler = g.sampler(FilterMode::linear, FilterMode::linear, MipmapFilterMode::nearest, AddressMode::clamp_to_edge, None, 1);
        let mut textures = [0; TABLES];
        let mut views = [0; TABLES];
        for k in 0..TABLES {
            let (w, h) = SIZES[k];
            textures[k] = g.texture(w, h, 1, Format::rgba16float, 1, texture_usage::TEXTURE_BINDING | texture_usage::RENDER_ATTACHMENT);
            views[k] = g.view(textures[k], false, 0, 1, 0, 1);
        }
        let passes = [
            PIPELINE_ATMOS_TRANSMITTANCE,
            PIPELINE_ATMOS_MULTISCATTER,
            PIPELINE_ATMOS_SKY_VIEW,
            PIPELINE_ATMOS_EXPOSURE,
            PIPELINE_ATMOS_IRRADIANCE,
        ]
        .map(|id| pipelines.build(g, id, Format::rgba16float));

        let mut atmosphere = Atmosphere {
            uniform,
            words,
            sampler,
            views,
            passes,
            groups: [0; TABLES],
            baked: false,
            drawn_for: [None; 2],
            slopes: (
                g.buffer(8, buffer_usage::STORAGE | buffer_usage::COPY_SRC | buffer_usage::COPY_DST),
                g.buffer(8, buffer_usage::MAP_READ | buffer_usage::COPY_DST),
            ),
            slopes_of: [false; 2],
            landed: [0; 2],
            landed_word: 0,
            steps: [None; 2],
            sun_mu: 1.0,
            moon_mu: 1.0,
            earth: ImageMap::new(g, EARTH_SIZE.0, EARTH_SIZE.1),
            earth_tiles: {
                let texture = g.texture(
                    TILE_CELLS,
                    TILE_CELLS,
                    EARTH_LAYERS,
                    Format::rgba8unorm_srgb,
                    1,
                    texture_usage::TEXTURE_BINDING | texture_usage::COPY_DST,
                );
                let view = g.view(texture, true, 0, 1, 0, EARTH_LAYERS);
                (texture, view)
            },
        };
        // Every table's group is the air's entries it reads and its own.
        let air = atmosphere.entries();
        let of = |bindings: &[u32], own: &[[u32; 5]]| -> Vec<u32> {
            let shared = air.as_chunks::<5>().0.iter().filter(|c| bindings.contains(&c[0]));
            shared.chain(own).flatten().copied().collect()
        };
        let medium = [8, 9, 11];
        let multiscatter = [12, resource::VIEW, views[MULTISCATTER], 0, 0];
        let frame = buffer_entry(0, frame_buffer);
        let light = |which: f32| {
            let light = g.buffer(16, buffer_usage::UNIFORM | buffer_usage::COPY_DST);
            g.write_buffer(light, 0, bytes(&[which, 0.0, 0.0, 0.0]));
            buffer_entry(14, light)
        };
        // The exposure reads the air but what it writes.
        let unexposed: Vec<u32> = air.as_chunks::<5>().0.iter().map(|c| c[0]).filter(|&b| b != 13).collect();
        let reads: [(usize, Vec<u32>); TABLES] = [
            (TRANSMITTANCE, of(&[8], &[])),
            (MULTISCATTER, of(&medium, &[])),
            (IRRADIANCE, of(&medium, &[multiscatter])),
            // The views are measured from the frame's up, and take their
            // slopes off the land's light too.
            (SKY_VIEW, of(&[8, 9, 11, 18], &[multiscatter, light(0.0), frame, buffer_entry(20, atmosphere.slopes.0)])),
            (MOON_VIEW, of(&[8, 9, 11, 18], &[multiscatter, light(1.0), frame, buffer_entry(20, atmosphere.slopes.0)])),
            (EXPOSURE_TEXEL, of(&unexposed, &[frame])),
        ];
        for (k, entries) in reads {
            atmosphere.groups[k] = g.bind_group(atmosphere.passes[pass_of(k)].layouts[0], &entries);
        }
        atmosphere
    }

    pub fn entries(&self) -> [u32; AIR_WORDS] {
        let mut e = [0u32; AIR_WORDS];
        e[..5].copy_from_slice(&buffer_entry(8, self.uniform));
        e[5..10].copy_from_slice(&[9, resource::VIEW, self.views[TRANSMITTANCE], 0, 0]);
        e[10..15].copy_from_slice(&[10, resource::VIEW, self.views[SKY_VIEW], 0, 0]);
        e[15..20].copy_from_slice(&[11, resource::SAMPLER, self.sampler, 0, 0]);
        e[20..25].copy_from_slice(&[13, resource::VIEW, self.views[EXPOSURE_TEXEL], 0, 0]);
        e[25..30].copy_from_slice(&[15, resource::VIEW, self.views[MOON_VIEW], 0, 0]);
        e[30..35].copy_from_slice(&[16, resource::VIEW, self.earth.view, 0, 0]);
        e[35..40].copy_from_slice(&[17, resource::VIEW, self.earth_tiles.1, 0, 0]);
        e[40..45].copy_from_slice(&[18, resource::VIEW, self.views[IRRADIANCE], 0, 0]);
        e[45..50].copy_from_slice(&[12, resource::VIEW, self.views[MULTISCATTER], 0, 0]);
        e
    }

    /// The earth layer's tiles: what has come goes into its layer as the
    /// stream has room, and the window says which layer each tile reads.
    /// Written by `lay_out`, which the renderer calls after this; whether a
    /// tile came this frame.
    pub fn set_earth_tiles(&mut self, g: &Gpu, window: [i32; 2], layers: &[u32], upload: &mut Upload) -> bool {
        let (texture, mut came) = (self.earth_tiles.0, false);
        upload(&mut |layer, texels| {
            if !room(texels.len() as u32) {
                return false;
            }
            g.write_texture(texture, layer, TILE_CELLS, TILE_CELLS, texels);
            came = true;
            true
        });
        let w = &mut self.words;
        w[EARTH_WINDOW] = f32::from_bits(window[0] as u32);
        w[EARTH_WINDOW + 1] = f32::from_bits(window[1] as u32);
        let per_word = (32 / TILE_BITS) as usize;
        let mut packed = [0u32; 4];
        for (k, &layer) in layers.iter().enumerate() {
            packed[k / per_word] |= layer << (TILE_BITS * (k % per_word) as u32);
        }
        for (w, p) in w[EARTH_TILES..EARTH_TILES + 4].iter_mut().zip(packed) {
            *w = f32::from_bits(p);
        }
        came
    }

    /// This month's Earth: the kernel draws nothing before its image has
    /// come; whether it came this frame.
    pub fn earth_step(&mut self, g: &Gpu, mips: &Mips, image: fn() -> Option<u32>) -> bool {
        self.earth.step(g, mips, image)
    }

    /// The words the sky and the world read, as `lay_out` wrote them.
    pub fn words(&self) -> &[f32] {
        &self.words
    }

    /// `up` is straight up where the eye stands, which every light and ray height is measured from.
    pub fn lay_out(&mut self, g: &Gpu, env: &[f32], up: &V3) {
        let (sun, moon) = (&env[ENV_SUN..ENV_SUN + 3], &env[ENV_MOON..ENV_MOON + 4]);
        let altitude_km = env[ENV_AIR];
        self.words[SUN_DIR..SUN_DIR + 3].copy_from_slice(sun);
        self.words[MOON_DIR..MOON_DIR + 3].copy_from_slice(&moon[..3]);
        self.words[MOON_LIGHT] = moon[3];
        self.words[CAMERA_ALTITUDE] = altitude_km;
        self.words[KM_PER_U] = env[ENV_AIR + 1];
        self.words[LAND_ALTITUDE] = env[ENV_AIR + 3];
        self.words[CENTER_LON] = env[ENV_SKY + 2];
        let height = |d: &[f32]| (d[0] as f64 * up[0] + d[1] as f64 * up[1] + d[2] as f64 * up[2]) as f32;
        (self.sun_mu, self.moon_mu) = (height(sun), height(moon));
        g.write_buffer(self.uniform, 0, bytes(&self.words));
    }

    /// Whether a table the world reads was drawn again, and whether it
    /// recorded anything. A light's view is drawn again when its height or
    /// the eye's altitude has moved (`moved`), and only while no slopes are on
    /// their way, which it writes; it takes its light's slope as it is drawn.
    /// The exposure texel is a function of the words it reads and the tables,
    /// so it is drawn where `words_moved` says the words did, or a table was.
    pub fn draw(&mut self, g: &Gpu, encoder: u32, stamp: &mut Stamp, words_moved: bool) -> (bool, bool) {
        let mut drew = !self.baked;
        if !self.baked {
            self.table(g, encoder, TRANSMITTANCE, stamp);
            self.table(g, encoder, MULTISCATTER, stamp);
            self.table(g, encoder, IRRADIANCE, stamp);
            self.baked = true;
            self.drawn_for = [None; 2];
        }
        self.take_slopes();
        let altitude = self.words[CAMERA_ALTITUDE];
        let free = self.landed_word == 0;
        let views = [
            (SKY_VIEW, (self.sun_mu, altitude), true),
            (MOON_VIEW, (self.moon_mu, altitude), self.words[MOON_LIGHT] > 0.0),
        ];
        for (k, (table, now, lit)) in views.into_iter().enumerate() {
            if !(free && lit && moved(self.drawn_for[k], now)) {
                continue;
            }
            g.write_buffer(self.slopes.0, 4 * k as u32, bytes(&[0u32]));
            self.table(g, encoder, table, stamp);
            self.drawn_for[k] = Some(now);
            self.slopes_of[k] = true;
            drew = true;
        }
        if self.slopes_of.contains(&true) && free {
            g.copy_buffer(encoder, (self.slopes.0, 0), (self.slopes.1, 0), 8);
        }
        let exposed = drew || words_moved;
        if exposed {
            self.table(g, encoder, EXPOSURE_TEXEL, stamp);
        }
        (drew, exposed)
    }

    /// The slopes the views just drawn took, mapped back once their encoder
    /// is submitted; their landing wakes the turn that reads them.
    pub fn read_back(&mut self, g: &Gpu) {
        if self.slopes_of.contains(&true) && self.landed_word == 0 {
            let address = off_of(self.landed.as_ptr()) as u32;
            g.map_read(self.slopes.1, 0, 8, address, &mut self.landed_word, 1);
        }
    }

    /// Landed slopes become their lights' steps: the height over which the
    /// sky as drawn moves one display level at the slope it moves at now.
    fn take_slopes(&mut self) {
        if self.landed_word != 1 | DONE {
            return;
        }
        for k in 0..2 {
            if core::mem::take(&mut self.slopes_of[k]) {
                self.steps[k] = Some(1.0 / f32::from_bits(self.landed[k]));
            }
        }
        self.landed_word = 0;
    }

    /// Each light's step as the last landed slopes say (`take_slopes`), the
    /// sun's first; none before any landed.
    pub fn steps(&self) -> [Option<f32>; 2] {
        self.steps
    }

    fn table(&self, g: &Gpu, encoder: u32, k: usize, stamp: &mut Stamp) {
        let pass = g.pass(encoder, &pass_desc(self.views[k], Load::Clear, 0, Load::Clear, 0.0, stamp.pass()));
        g.set_pipeline(pass, self.passes[pass_of(k)].pipeline);
        g.set_bind_group(pass, 0, self.groups[k], &[]);
        g.draw(pass, 3, 1, 0, 0);
        g.end(pass);
    }
}

/// The moonlit sky is drawn by the sky view's pass.
fn pass_of(table: usize) -> usize {
    if table == MOON_VIEW {
        SKY_VIEW
    } else {
        table
    }
}

/// A view is drawn again for any new height of its light, which the sky's
/// clock moves only a step at a time (pxl-kernel scene/environment.rs, by
/// `steps`), or for the eye's altitude moved its share.
fn moved(drawn: Option<(f32, f32)>, now: (f32, f32)) -> bool {
    match drawn {
        None => true,
        Some((mu, h)) => mu != now.0 || (now.1 - h).abs() > (h.abs() * ALTITUDE_SHARE).max(ALTITUDE_FLOOR_KM),
    }
}
