// pxl-render/gpu/atmosphere.rs

//! The atmosphere's uniform and its tables: transmittance, multiple scattering
//! and the sky's irradiance once per device; the sun's and moon's sky views
//! redrawn only when the light's height or the camera's altitude moves (the
//! table is kept relative to the light's azimuth); and every frame the exposure.

use pxl_spec::bake::tile::earth::{self, EARTH_ZOOM};
use pxl_spec::space::layer::demand::{REACH_ACROSS, REACH_TILES};
use pxl_spec::space::layer::lattice::TILE_CELLS;

use pxl_spec::space::coord::world::around;

use crate::frame::shader::record::{
    COMPUTE, PIPELINE_ATMOS_EXPOSURE, PIPELINE_ATMOS_IRRADIANCE, PIPELINE_ATMOS_MULTISCATTER, PIPELINE_ATMOS_SKY_VIEW, PIPELINE_ATMOS_TRANSMITTANCE, SAMPLER, SHADER_ATMOS_FROXEL, STORAGE_TEXTURE_3D,
    TEXTURE, UNIFORM,
};
use xpute_gpu::enums::{AddressMode, FilterMode, MipmapFilterMode};

use xpute_guest::mem::off_of;

use crate::frame::shader::word::atmos::{
    BOTTOM_RADIUS, CAMERA_ALTITUDE, CITY_LIGHT, EARTH_REACH, EARTH_TILES, EARTH_WINDOW, EXPOSURE, GROUND_ALBEDO, KM_PER_U, LAND_ALTITUDE_KM, MIE_EXTINCTION, MIE_G, MIE_SCALE_HEIGHT, MIE_SCATTERING,
    MOON_COLOR, MOON_DIR, MOON_LIGHT, NIGHT_SKY, NIGHT_STOPS, OZONE_ABSORPTION, OZONE_CENTER, OZONE_HALF_WIDTH, RAYLEIGH_SCALE_HEIGHT, RAYLEIGH_SCATTERING, SHOWN_FALL, SUN_ANGULAR_RADIUS, SUN_DIR,
    TOP_RADIUS,
};
use crate::frame::shader::word::{AIR_FROXELS, ATMOS_FLOATS, EARTH_TILE_BITS};
use crate::gpu::api::{buffer_entry, buffer_usage, bytes, pass_desc, room, sampler_entry, texture_usage, view_entry, word_bytes, write_texture_bytes, Entry, Format, Gpu, Load, Stamp, DONE};
use crate::gpu::image_map::ImageMap;
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::read::Read;
use crate::look::atmosphere as model;
use crate::look::environment::{ENV_AIR, ENV_MOON, ENV_MOON_COLOR, ENV_SUN};

/// Hands each waiting tile's layer and texels to the writer while it takes them.
pub type Upload = dyn FnMut(&mut dyn FnMut(u32, &[u8]) -> bool);

/// Bindings 8 through 18, and the air's volume at 22: eleven entries.
pub const AIR_ENTRIES: usize = 11;

/// What the volume's maker binds: the frame, the volume it writes, the
/// air's words and the tables it marches through, and the sky's ray.
const FROXEL_GROUP: [u32; 24] = [
    0,
    COMPUTE,
    UNIFORM,
    1,
    COMPUTE,
    STORAGE_TEXTURE_3D,
    8,
    COMPUTE,
    UNIFORM,
    9,
    COMPUTE,
    TEXTURE,
    11,
    COMPUTE,
    SAMPLER,
    12,
    COMPUTE,
    TEXTURE,
    13,
    COMPUTE,
    TEXTURE,
    19,
    COMPUTE,
    UNIFORM,
];

/// A layer for each tile the earth layer's floor holds at once: its residency's frames.
pub const EARTH_LAYERS: u32 = REACH_TILES;
/// A tile's layer plus one in four bits, eight to a word.
const _: () = assert!(EARTH_LAYERS < 1 << EARTH_TILE_BITS && EARTH_LAYERS <= 32 / EARTH_TILE_BITS * 4, "the window's layers fit their word");

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
    /// The words as last laid out, and whether this frame's
    /// moved: what the uniform and the exposure texel stand on, read here
    /// alone.
    read: Read,
    moved: bool,
    /// This month's Earth: the kernel draws nothing before its image has
    /// come (`ImageMap::step`).
    pub earth: ImageMap,
    earth_tiles: (u32, u32),
    /// The air before each point of the view (atmos_froxel.wgsl), its maker
    /// and its maker's group; and what it was last made of: the frame's and
    /// the sky's words, the air's, and the eye's texel as it was drawn.
    volume: u32,
    froxel: Built,
    froxel_group: u32,
    air: Read,
}

impl Atmosphere {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines, frame_buffer: u32, sky_buffer: u32) -> Atmosphere {
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
        let n = AIR_FROXELS;
        // A slice past the air's, which holds each column's span (atmos_froxel.wgsl).
        let volume = g.volume_view(g.volume([n, n, n + 1], Format::rgba16float, texture_usage::STORAGE_BINDING | texture_usage::TEXTURE_BINDING));

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
            read: Read::default(),
            moved: false,
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
            volume,
            froxel: pipelines.build_compute(g, SHADER_ATMOS_FROXEL, &FROXEL_GROUP),
            froxel_group: 0,
            air: Read::default(),
        };
        // Every table's group is the air's entries it reads and its own.
        let air = atmosphere.entries();
        let of = |bindings: &[u32], own: &[Entry]| -> Vec<Entry> {
            let shared = air.iter().filter(|e| bindings.contains(&e.binding));
            shared.chain(own).copied().collect()
        };
        let medium = [8, 9, 11];
        let multiscatter = view_entry(12, views[MULTISCATTER]);
        let frame = buffer_entry(0, frame_buffer);
        let light = |which: f32| {
            let light = g.buffer(16, buffer_usage::UNIFORM | buffer_usage::COPY_DST);
            g.write_buffer(light, 0, bytes(&[which, 0.0, 0.0, 0.0]));
            buffer_entry(14, light)
        };
        // The exposure reads the air but what it writes, and the volume made of it.
        let unexposed: Vec<u32> = air.iter().map(|e| e.binding).filter(|&b| b != 13 && b != 22).collect();
        let reads: [(usize, Vec<Entry>); TABLES] = [
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
        let froxel = of(&[8, 9, 11, 12, 13], &[frame, view_entry(1, volume), buffer_entry(19, sky_buffer)]);
        atmosphere.froxel_group = g.bind_group(atmosphere.froxel.layouts[0], &froxel);
        atmosphere
    }

    pub fn entries(&self) -> [Entry; AIR_ENTRIES] {
        [
            buffer_entry(8, self.uniform),
            view_entry(9, self.views[TRANSMITTANCE]),
            view_entry(10, self.views[SKY_VIEW]),
            sampler_entry(11, self.sampler),
            view_entry(13, self.views[EXPOSURE_TEXEL]),
            view_entry(15, self.views[MOON_VIEW]),
            view_entry(16, self.earth.view),
            view_entry(17, self.earth_tiles.1),
            view_entry(18, self.views[IRRADIANCE]),
            view_entry(12, self.views[MULTISCATTER]),
            view_entry(22, self.volume),
        ]
    }

    /// The air before each point of the view made again where what it is
    /// made of moved (`air`): the frame's words, the sky's, the air's, or the
    /// eye's texel drawn this frame (`exposed`); whether it was.
    pub fn froxel(&mut self, g: &Gpu, encoder: u32, [frame, sky]: [&[f32]; 2], exposed: bool, stamp: &mut Stamp) -> bool {
        let r = &mut self.air;
        r.begin();
        r.read(frame);
        r.read(sky);
        r.read(&self.words);
        r.mark(exposed);
        if r.end() {
            return false;
        }
        let pass = g.compute_pass(encoder, stamp.pass());
        g.set_pipeline(pass, self.froxel.pipeline);
        g.set_bind_group(pass, 0, self.froxel_group, &[]);
        // A column of froxels a workgroup (atmos_froxel.wgsl).
        g.dispatch(pass, AIR_FROXELS, AIR_FROXELS, 1);
        g.end(pass);
        true
    }

    /// The earth layer's tiles: what has come goes into its layer as the
    /// stream has room, and the window says which layer each tile reads.
    /// Written by `lay_out`, which the renderer calls after this; whether a
    /// tile came this frame.
    pub fn set_earth_tiles(&mut self, g: &Gpu, window: [i32; 2], layers: &[u32], upload: &mut Upload) -> bool {
        let (texture, mut came) = (self.earth_tiles.0, false);
        upload(&mut |layer, texels| {
            if !room(write_texture_bytes(texels)) {
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

    /// The words the sky and the world read, as `lay_out` wrote them.
    pub fn words(&self) -> &[f32] {
        &self.words
    }

    /// Every light's and ray's height is measured up the world's z, straight
    /// up where the eye stands.
    pub fn lay_out(&mut self, g: &Gpu, env: &[f32]) {
        let (sun, moon) = (&env[ENV_SUN..ENV_SUN + 3], &env[ENV_MOON..ENV_MOON + 4]);
        let altitude_km = env[ENV_AIR];
        self.words[SUN_DIR..SUN_DIR + 3].copy_from_slice(sun);
        self.words[MOON_DIR..MOON_DIR + 3].copy_from_slice(&moon[..3]);
        self.words[MOON_LIGHT] = moon[3];
        self.words[MOON_COLOR..MOON_COLOR + 3].copy_from_slice(&env[ENV_MOON_COLOR..ENV_MOON_COLOR + 3]);
        self.words[CAMERA_ALTITUDE] = altitude_km;
        self.words[KM_PER_U] = env[ENV_AIR + 1];
        self.words[LAND_ALTITUDE_KM] = env[ENV_AIR + 3];
        (self.sun_mu, self.moon_mu) = (sun[2], moon[2]);
        let r = &mut self.read;
        r.begin();
        r.read(&self.words);
        self.moved = !r.end();
        if self.moved {
            g.write_buffer(self.uniform, 0, bytes(&self.words));
        }
    }

    /// Whether a table the world reads was drawn again, and whether it
    /// recorded anything. A light's view is drawn again when its height or
    /// the eye's altitude has moved (`moved`), and only while no slopes are on
    /// their way, which it writes; it takes its light's slope as it is drawn.
    /// The exposure texel is a function of the words it reads and the
    /// tables, so it is drawn where `lay_out` found the words moved, or a
    /// table was drawn.
    pub fn draw(&mut self, g: &Gpu, encoder: u32, stamp: &mut Stamp) -> (bool, bool) {
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
        let views = [(SKY_VIEW, (self.sun_mu, altitude), true), (MOON_VIEW, (self.moon_mu, altitude), self.words[MOON_LIGHT] > 0.0)];
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
        let exposed = drew || self.moved;
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
/// clock moves only a step at a time (pxl-kernel domain/scene/environment.rs, by
/// `steps`), or for the eye's altitude moved its share.
fn moved(drawn: Option<(f32, f32)>, now: (f32, f32)) -> bool {
    match drawn {
        None => true,
        Some((mu, h)) => mu != now.0 || (now.1 - h).abs() > (h.abs() * ALTITUDE_SHARE).max(ALTITUDE_FLOOR_KM),
    }
}
