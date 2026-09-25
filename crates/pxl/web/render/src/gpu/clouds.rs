// pxl-render/gpu/clouds.rs

//! The clouds: a layer over the planet drawn after the opaque world
//! (frame/shader/clouds.wgsl), one full-screen pass reading the frame, the
//! air, and where the camera stands on the ground. What it costs is a
//! fragment a pixel where a ray meets the layer — eight octaves of value noise
//! and the air's lookups — and nothing where one does not.

use pxl_spec::space::mat4::Mat4;
use pxl_spec::space::pvf3::core::{IVEC_X_W, IVEC_Y_W};
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::v3::V3;

use crate::frame::shader::record::{PIPELINE_CLOUDS, PIPELINE_CLOUDS_ABOVE, PIPELINE_CLOUD_FIELD};
use crate::gpu::api::{buffer_entry, buffer_usage, bytes, object, pass_desc, release, texture_usage, word_bytes, Gpu, Stamp};
use crate::gpu::api_spec::{address, compare, dimension, filter, format, load, resource};
use crate::gpu::atmosphere::AIR_WORDS;
use crate::gpu::layers::{mip_levels, Mips};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::renderer::put_ray;
use crate::look::atmosphere::light_at;
use crate::look::environment::{ENV_AIR, ENV_MOON, ENV_SKY, ENV_SUN};

/// Floats the Clouds uniform (clouds.wgsl's Clouds) holds: the ray, and the
/// sun's and the moon's light at the layer. Where the layer is and what
/// shapes it is the air's uniform's (`field`), beside the height of the
/// camera it is shown from.
pub(crate) const CLOUDS_FLOATS: usize = 12 + 8;
pub(crate) const SUN_AT: usize = 12;
pub(crate) const MOON_AT: usize = 16;
/// Where the layer is and what shapes it, as the air's uniform carries it
/// (atmos_medium.wgsl's `cloud_*`, frame/shader/cloud_field.wgsl).
pub struct CloudField {
    /// The lattice index the root frame's origin lies in, and the fraction
    /// of a unit past it.
    pub base: [u32; 2],
    pub frac: [f32; 2],
    /// The sky's clock, in seconds: the wind carries the layer by it.
    pub time: f32,
    /// The lattice's count across the world, where it wraps.
    pub count: u32,
    /// The layer's height, and the camera's height past which it is gone.
    pub altitude_km: f32,
    pub gone_km: f32,
}
/// The layer's height: a fair-weather cumulus base.
const ALTITUDE_KM: f64 = 1.5;
/// The camera's height past which the layer is not drawn, faded out from
/// the layer up: seen from above, flat noise over the ground reads as blots
/// on it. Four times the layer's own height.
const GONE_KM: f64 = 4.0 * ALTITUDE_KM;
/// The tile the layer's shape is baked into once for a device
/// (frame/shader/cloud_tile.wgsl): its lattice units across, and its texels.
/// One fetch a pixel, and its mip chain is what a pixel too small for the
/// finest octave reads, in place of the eight octaves of hashes the shape
/// takes to make.
pub(crate) const TILE_UNITS: u32 = 32;
pub(crate) const TILE_TEXELS: u32 = 1024;
// A unit is a whole number of texels, so a lattice unit's fraction is a texel
// coordinate exactly and the tile meets itself without a seam.
/// The cells a unit of the clouds' lattice spans, as a shift
/// (clouds.wgsl's CLOUD_LATTICE_CELLS).
pub(crate) const LATTICE_SHAMT: u32 = 12;
/// The lattice's count across the world, where it wraps: the world is the
/// X lane's width in cells (space/adapter/3857.rs).
const LATTICE_COUNT: u32 = 1 << (IVEC_X_W - LATTICE_SHAMT);
// Y's lane is as wide, so one count wraps both axes.
const _: () = assert!(IVEC_X_W == IVEC_Y_W, "one lattice count for both axes");
// The tile meets itself where the lattice wraps as well as where the tile
// does, so the world is a whole number of tiles across.
const _: () = assert!(LATTICE_COUNT.is_multiple_of(TILE_UNITS), "the world is whole tiles");

pub struct CloudPass {
    /// From under the layer, drawn only where the world left the far plane;
    /// from over it, over everything (frame/shader/clouds.wgsl).
    below: Built,
    above: Built,
    /// The camera over the layer this frame, and past where it is gone.
    over: bool,
    gone: bool,
    uniform: u32,
    group: u32,
    words: [f32; CLOUDS_FLOATS],
}

impl CloudPass {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines, frame_buffer: u32, air: [u32; AIR_WORDS]) -> CloudPass {
        let below = pipelines.build(g, PIPELINE_CLOUDS, format::CANVAS);
        let above = pipelines.build(g, PIPELINE_CLOUDS_ABOVE, format::CANVAS);
        let tile = object();
        let tile_levels = mip_levels(TILE_TEXELS, TILE_TEXELS);
        g.texture(
            tile,
            TILE_TEXELS,
            TILE_TEXELS,
            1,
            format::R8,
            tile_levels,
            texture_usage::TEXTURE_BINDING | texture_usage::RENDER_ATTACHMENT,
        );
        let tile_view = object();
        g.view(tile_view, tile, dimension::D2, 0, tile_levels, 0, 1);
        // Repeated, since the tile meets itself, and filtered between its
        // levels as well as within one: a pixel reads the level its
        // footprint asks for.
        let tile_sampler = object();
        g.sampler(tile_sampler, filter::LINEAR, filter::LINEAR, filter::LINEAR, address::REPEAT, compare::NONE, 1);
        let uniform = object();
        g.buffer(uniform, word_bytes(CLOUDS_FLOATS), buffer_usage::UNIFORM | buffer_usage::COPY_DST);
        let group = object();
        let mut e = [0u32; 20 + AIR_WORDS];
        e[..5].copy_from_slice(&buffer_entry(0, frame_buffer));
        e[5..10].copy_from_slice(&buffer_entry(1, uniform));
        e[10..15].copy_from_slice(&[2, resource::VIEW, tile_view, 0, 0]);
        e[15..20].copy_from_slice(&[3, resource::SAMPLER, tile_sampler, 0, 0]);
        e[20..].copy_from_slice(&air);
        // One layout serves both pipelines: their groups are the same.
        g.bind_group(group, below.layouts[0], &e);
        bake(g, pipelines, tile, tile_levels);
        CloudPass {
            below,
            above,
            over: false,
            gone: false,
            uniform,
            group,
            words: [0.0; CLOUDS_FLOATS],
        }
    }

    /// The frame's clouds: `view_proj`, the root frame to WebGPU clip, and
    /// the eye in it; the environment's lane `env`, for the sun's and the
    /// moon's directions, whose light reaches the layer through the air; and
    /// straight up where the eye stands, which their heights are measured
    /// from.
    pub fn lay_out(&mut self, g: &Gpu, view_proj: &Mat4, eye: &V3, env: &[f32], up: &V3) {
        put_ray(&mut self.words, 0, view_proj, eye);
        self.over = env[ENV_AIR] as f64 > ALTITUDE_KM;
        self.gone = env[ENV_AIR] as f64 >= GONE_KM;
        for (at, toward) in [(SUN_AT, ENV_SUN), (MOON_AT, ENV_MOON)] {
            let mu = (0..3).map(|k| env[toward + k] as f64 * up[k]).sum();
            for (w, v) in self.words[at..at + 3].iter_mut().zip(light_at(ALTITUDE_KM, mu)) {
                *w = v as f32;
            }
        }
        g.write_buffer(self.uniform, 0, bytes(&self.words));
    }

    /// The layer into the open pass, over what is drawn opaque.
    pub fn draw(&self, g: &Gpu, pass: u32) {
        if self.gone {
            return;
        }
        g.set_pipeline(pass, if self.over { self.above.pipeline } else { self.below.pipeline });
        g.set_bind_group(pass, 0, self.group, &[]);
        g.draw(pass, 3, 1, 0, 0);
    }
}

/// Where the layer is and what shapes it, for the air's uniform
/// (gpu/atmosphere.rs's CLOUD_BASE on): the lattice index the root frame's
/// origin — the cell it is on the map, as the integer lanes of its PVF3 —
/// lies in and the fraction of a unit past it; the sky's clock, from the
/// environment's lane `env`; the lattice's count across the world; the
/// layer's height; and the camera's height past which it is gone.
///
/// The origin is split on the lattice here, in its own lanes, so the
/// shader never holds a coordinate the size of the world: it gets the
/// lattice index, the lane shifted down, and the fraction of a unit past
/// it, the lane's low bits, and adds only the few thousand cells between
/// the origin and the layer. A shift and a mask on two's complement lanes
/// are a floor and a remainder, so a cell west of the origin's zero lands
/// in the lattice unit it is in, and the index masked to the count is the
/// same unit wherever the lane wrapped.
pub fn field(origin: IVec, env: &[f32]) -> CloudField {
    let unit = ivec::sh(origin, -(LATTICE_SHAMT as i32));
    let within = ivec::and(origin, ivec::fill((1 << LATTICE_SHAMT) - 1));
    let (base, frac) = (
        [ivec::X, ivec::Y].map(|lane| lane(unit) as u32 & (LATTICE_COUNT - 1)),
        [ivec::X, ivec::Y].map(|lane| lane(within) as f32 / (1u32 << LATTICE_SHAMT) as f32),
    );
    CloudField {
        base,
        frac,
        time: env[ENV_SKY + 1],
        count: LATTICE_COUNT,
        altitude_km: ALTITUDE_KM as f32,
        gone_km: GONE_KM as f32,
    }
}

/// Draws the tile the shape is read from, with its mip chain: one field over
/// the whole map, so it is made once for the device
/// (frame/shader/cloud_bake.wgsl) and a pixel reads a texel of it in place of
/// the eight octaves of hashes it takes to make.
fn bake(g: &Gpu, pipelines: &mut Pipelines, tile: u32, levels: u32) {
    let built = pipelines.build(g, PIPELINE_CLOUD_FIELD, format::R8);
    let encoder = object();
    g.encoder(encoder);
    let level = object();
    g.view(level, tile, dimension::D2, 0, 1, 0, 1);
    let pass = object();
    g.pass(pass, encoder, &pass_desc(level, load::CLEAR, 0, load::CLEAR, 0.0, Stamp::UNTIMED));
    g.set_pipeline(pass, built.pipeline);
    g.draw(pass, 3, 1, 0, 0);
    g.end(pass);
    for id in [pass, level] {
        release(id);
    }
    g.submit(encoder);
    release(encoder);
    Mips::new(g, pipelines, format::R8).build(g, tile, levels, 0, 1);
}
