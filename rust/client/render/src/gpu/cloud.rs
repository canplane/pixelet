// pxl-render/gpu/cloud.rs

//! The cloud sheets, altocumulus and cirrus, each a plane drawn over the
//! world's pass against its depth, both reading the cloud field (`pxl-spec-core cloud.rs`) as a tile baked once
//! per device.
//! The cumulus is drawn as decor is (`pxl-kernel domain/scene/environment/cloud.rs`).

use crate::frame::shader::record::{PIPELINE_CLOUD, PIPELINE_CLOUD_FIELD};
use xpute_gpu::enums::{AddressMode, FilterMode, MipmapFilterMode};

use crate::frame::shader::word::clouds::{BASE, COUNT, DRIFT, FRAC, REACH, SHOWN};
use crate::frame::shader::word::{CLOUDS_FLOATS, CLOUD_TILE_TEXELS, CLOUD_TILE_UNITS};
use crate::gpu::api::{
    buf_entry, buffer_usage, bytes, pass_desc, release, sampler_entry, texture_usage, view_entry,
    word_bytes, Entry, Format, Gpu, Load,
};
use crate::gpu::atmosphere::AIR_ENTRIES;
use crate::gpu::mip::{mip_levels, Mips};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::surface::WORLD_COLOR;
use crate::look::environment::ENV_SKY;
use pxl_spec::space::coord::world::EXTENT;
use pxl_spec_core::cloud::{drift, LATTICE_CNT};
use pxl_spec_core::param::cloud::LATTICE_SHAMT;
use pxl_spec_core::UNIT_ZOOM;

const _: () = assert!(
    EXTENT[0] == UNIT_ZOOM && EXTENT[1] == UNIT_ZOOM,
    "one lattice count for both axes"
);
const _: () = assert!(
    LATTICE_CNT.is_multiple_of(CLOUD_TILE_UNITS),
    "the world is whole tiles"
);

pub struct CloudPass {
    built: Built,
    uniform: u32,
    group: u32,
    words: [f32; CLOUDS_FLOATS],
}

impl CloudPass {
    pub fn new(
        g: &Gpu,
        pipelines: &mut Pipelines,
        frame_buf: u32,
        air: [Entry; AIR_ENTRIES],
    ) -> CloudPass {
        let built = pipelines.build(g, PIPELINE_CLOUD, WORLD_COLOR);
        let tile = g.texture(
            CLOUD_TILE_TEXELS,
            CLOUD_TILE_TEXELS,
            1,
            Format::r8unorm,
            mip_levels(CLOUD_TILE_TEXELS, CLOUD_TILE_TEXELS),
            texture_usage::TEXTURE_BINDING | texture_usage::RENDER_ATTACHMENT,
        );
        let tile_view = g.view(
            tile,
            false,
            0,
            mip_levels(CLOUD_TILE_TEXELS, CLOUD_TILE_TEXELS),
            0,
            1,
        );
        // Repeated, since the tile meets itself.
        let tile_sampler = g.sampler(
            FilterMode::linear,
            FilterMode::linear,
            MipmapFilterMode::linear,
            AddressMode::repeat,
            None,
            1,
        );
        let uniform = g.buf(
            word_bytes(CLOUDS_FLOATS),
            buffer_usage::UNIFORM | buffer_usage::COPY_DST,
        );
        let mut e = vec![
            buf_entry(0, frame_buf),
            buf_entry(1, uniform),
            view_entry(2, tile_view),
            sampler_entry(3, tile_sampler),
        ];
        e.extend(air);
        let group = g.bind_group(built.layouts[0], &e);
        bake(
            g,
            pipelines,
            tile,
            mip_levels(CLOUD_TILE_TEXELS, CLOUD_TILE_TEXELS),
        );
        CloudPass {
            built,
            uniform,
            group,
            words: [0.0; CLOUDS_FLOATS],
        }
    }

    /// `shown`, how much of the clouds the kernel shows; `over`, the eye's
    /// place on the map in the world's doubles; `reach_km` how far it
    /// draws: past it no sheet is drawn. The field moves with the wind,
    /// so its words are the clouds' own.
    pub fn lay_out(&mut self, over: [f64; 2], env: &[f32], shown: f64, reach_km: f32) {
        self.words[SHOWN] = shown as f32;
        self.words[REACH] = reach_km;
        field(&mut self.words, over, env);
    }

    /// `words` written, where the frame's read found them moved.
    pub fn write(&self, g: &Gpu) {
        g.write_buf(self.uniform, 0, bytes(&self.words));
    }

    /// The sheets, over the world's pass.
    /// The words its pass reads, as `lay_out` wrote them.
    pub fn words(&self) -> &[f32] {
        &self.words
    }

    pub fn draw(&self, g: &Gpu, pass: u32) {
        if self.words[SHOWN] <= 0.0 {
            return;
        }
        g.set_pipeline(pass, self.built.pipeline);
        g.set_bind_group(pass, 0, self.group, &[]);
        // A quad each, the farther sheet first (cloud.wgsl `vs`).
        g.draw(pass, 6, 2, 0, 0);
    }
}

/// The eye, in the world's doubles, split on the lattice into a whole unit
/// and a fraction of one, so the shader never holds a world-sized coordinate;
/// the world is `LATTICE_COUNT` units around, so a lap of X is a whole number
/// of them and the unit wraps with it. And where the wind has carried the
/// field by now.
fn field(words: &mut [f32; CLOUDS_FLOATS], eye: [f64; 2], env: &[f32]) {
    let side = (1u32 << LATTICE_SHAMT) as f64;
    let drift = drift(f64::from(env[ENV_SKY + 1]));
    for k in 0..2 {
        let u = eye[k] / side;
        words[BASE + k] =
            f32::from_bits((u.floor() as i64).rem_euclid(i64::from(LATTICE_CNT)) as u32);
        words[FRAC + k] = (u - u.floor()) as f32;
        words[DRIFT + k] = drift[k] as f32;
    }
    words[COUNT] = f32::from_bits(LATTICE_CNT);
}

fn bake(g: &Gpu, pipelines: &mut Pipelines, tile: u32, levels: u32) {
    let built = pipelines.build(g, PIPELINE_CLOUD_FIELD, Format::r8unorm);
    let encoder = g.encoder();
    let level = g.view(tile, false, 0, 1, 0, 1);
    let pass = g.pass(
        encoder,
        &pass_desc(level, Load::Clear, 0, Load::Clear, 0.0, None),
    );
    g.set_pipeline(pass, built.pipeline);
    g.draw(pass, 3, 1, 0, 0);
    g.end(pass);
    release(level);
    g.submit(encoder);
    let mips = Mips::new(g, pipelines, Format::r8unorm);
    mips.build(g, tile, levels, 0, 1);
    mips.release();
}
