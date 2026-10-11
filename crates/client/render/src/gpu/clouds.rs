// pxl-render/gpu/clouds.rs

//! The cloud sheets, altocumulus and cirrus, laid over the world's pass, both
//! reading the cloud field (`look/cloud.rs`) as a tile baked once per device.
//! The cumulus is drawn as decor is (`pxl-kernel scene/environment/cloud.rs`).

use pxl_spec::space::coord::mat4::Mat4;
use pxl_spec::space::coord::v3::V3;

use crate::frame::shader::record::{PIPELINE_CLOUDS, PIPELINE_CLOUDS_ABOVE, PIPELINE_CLOUD_FIELD};
use crate::gpu::api::{buffer_entry, buffer_usage, bytes, object, pass_desc, release, texture_usage, word_bytes, Gpu, Stamp};
use crate::gpu::api_spec::{address, compare, dimension, filter, format, load, resource};
use crate::gpu::atmosphere::AIR_WORDS;
use crate::gpu::layers::{mip_levels, Mips};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::renderer::put_ray;
use crate::look::cloud::{drift, CUMULUS_KM, LATTICE_COUNT, LATTICE_SHAMT, TILE_TEXELS};
use crate::look::environment::{ENV_AIR, ENV_SKY};

/// The ray, its first word's last lane how much of the clouds shows.
pub(crate) const CLOUDS_FLOATS: usize = 12;
const SHOWN: usize = 3;
pub struct CloudField {
    pub base: [u32; 2],
    pub frac: [f32; 2],
    pub drift: [f32; 2],
    pub count: u32,
    pub altitude_km: f32,
}

pub struct CloudPass {
    below: Built,
    above: Built,
    over: bool,
    uniform: u32,
    group: u32,
    words: [f32; CLOUDS_FLOATS],
}

impl CloudPass {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines, frame_buffer: u32, air: [u32; AIR_WORDS]) -> CloudPass {
        let below = pipelines.build(g, PIPELINE_CLOUDS, format::CANVAS);
        let above = pipelines.build(g, PIPELINE_CLOUDS_ABOVE, format::CANVAS);
        let tile = object();
        g.texture(
            tile,
            TILE_TEXELS,
            TILE_TEXELS,
            1,
            format::R8,
            mip_levels(TILE_TEXELS, TILE_TEXELS),
            texture_usage::TEXTURE_BINDING | texture_usage::RENDER_ATTACHMENT,
        );
        let tile_view = object();
        g.view(tile_view, tile, dimension::D2, 0, mip_levels(TILE_TEXELS, TILE_TEXELS), 0, 1);
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
        bake(g, pipelines, tile, mip_levels(TILE_TEXELS, TILE_TEXELS));
        CloudPass {
            below,
            above,
            over: false,
            uniform,
            group,
            words: [0.0; CLOUDS_FLOATS],
        }
    }

    /// `shown`, how much of the clouds the kernel shows.
    pub fn lay_out(&mut self, g: &Gpu, view_proj: &Mat4, eye: &V3, env: &[f32], shown: f64) {
        put_ray(&mut self.words, 0, view_proj, eye);
        self.words[SHOWN] = shown as f32;
        self.over = env[ENV_AIR] as f64 > CUMULUS_KM;
        g.write_buffer(self.uniform, 0, bytes(&self.words));
    }

    /// The sheets, over the world's pass.
    pub fn draw(&self, g: &Gpu, pass: u32) {
        if self.words[SHOWN] <= 0.0 {
            return;
        }
        g.set_pipeline(pass, if self.over { self.above.pipeline } else { self.below.pipeline });
        g.set_bind_group(pass, 0, self.group, &[]);
        g.draw(pass, 3, 1, 0, 0);
    }
}

/// The eye, in the world's doubles, split on the lattice into a whole unit
/// and a fraction of one, so the shader never holds a world-sized coordinate.
/// The world is `LATTICE_COUNT` units around, so a lap of X is a whole number
/// of them and the unit wraps with it.
pub fn field(eye: [f64; 2], env: &[f32]) -> CloudField {
    let side = (1u32 << LATTICE_SHAMT) as f64;
    let units = eye.map(|v| v / side);
    let (base, frac) = (units.map(|u| (u.floor() as i64).rem_euclid(i64::from(LATTICE_COUNT)) as u32), units.map(|u| (u - u.floor()) as f32));
    CloudField {
        base,
        frac,
        drift: drift(f64::from(env[ENV_SKY + 1])).map(|d| d as f32),
        count: LATTICE_COUNT,
        altitude_km: CUMULUS_KM as f32,
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
    let mips = Mips::new(g, pipelines, format::R8);
    mips.build(g, tile, levels, 0, 1);
    mips.release();
}
