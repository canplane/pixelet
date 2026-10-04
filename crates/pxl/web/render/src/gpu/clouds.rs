// pxl-render/gpu/clouds.rs

//! The cloud layer: one full-screen pass after the opaque world, reading a
//! tile baked once per device.

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

/// The ray, and the sun's and moon's light at the layer.
pub(crate) const CLOUDS_FLOATS: usize = 12 + 8;
pub(crate) const SUN_AT: usize = 12;
pub(crate) const MOON_AT: usize = 16;
pub struct CloudField {
    pub base: [u32; 2],
    pub frac: [f32; 2],
    pub time: f32,
    pub count: u32,
    pub altitude_km: f32,
    pub gone_km: f32,
}
/// A fair-weather cumulus base.
const ALTITUDE_KM: f64 = 1.5;
/// Seen from above, flat noise over the ground reads as blots.
const GONE_KM: f64 = 4.0 * ALTITUDE_KM;
/// One fetch a pixel and a mip chain, in place of eight octaves of hashes.
pub(crate) const TILE_UNITS: u32 = 32;
pub(crate) const TILE_TEXELS: u32 = 1024;
/// A unit is a whole number of texels, so a unit's fraction is a texel
/// coordinate exactly and the tile meets itself without a seam.
pub(crate) const LATTICE_SHAMT: u32 = 12;
const LATTICE_COUNT: u32 = 1 << (IVEC_X_W - LATTICE_SHAMT);
const _: () = assert!(IVEC_X_W == IVEC_Y_W, "one lattice count for both axes");
const _: () = assert!(LATTICE_COUNT.is_multiple_of(TILE_UNITS), "the world is whole tiles");

pub struct CloudPass {
    below: Built,
    above: Built,
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
        // Repeated, since the tile meets itself.
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

    pub fn draw(&self, g: &Gpu, pass: u32) {
        if self.gone {
            return;
        }
        g.set_pipeline(pass, if self.over { self.above.pipeline } else { self.below.pipeline });
        g.set_bind_group(pass, 0, self.group, &[]);
        g.draw(pass, 3, 1, 0, 0);
    }
}

/// The origin is split on the lattice in integer lanes, so the shader never
/// holds a world-sized coordinate. Shift and mask on two's complement are a
/// floor and a remainder, so cells west of zero land in the right unit.
pub fn field(origin: IVec, past: [f64; 2], env: &[f32]) -> CloudField {
    let unit = ivec::sh(origin, -(LATTICE_SHAMT as i32));
    let within = ivec::and(origin, ivec::fill((1 << LATTICE_SHAMT) - 1));
    let side = (1u32 << LATTICE_SHAMT) as f64;
    let at = [ivec::X, ivec::Y].map(|lane| lane(within) as f64);
    let units: [f64; 2] = core::array::from_fn(|k| (at[k] + past[k]) / side);
    let (base, frac) = (
        core::array::from_fn(|k| ([ivec::X, ivec::Y][k](unit) + units[k].floor() as i64) as u32 & (LATTICE_COUNT - 1)),
        core::array::from_fn(|k| (units[k] - units[k].floor()) as f32),
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
