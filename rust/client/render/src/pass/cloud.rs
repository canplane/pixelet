// pxl-render/pass/cloud.rs

//! Draws the two cloud sheets, altocumulus and cirrus. Each sheet is a plane drawn in the
//! world's pass against the world's depth. Both read the cloud field (pxl-content cloud.rs)
//! from one tile, which is baked once when the renderer starts and repeats across the world.
//! The cumulus is not drawn here: it is drawn as decor is (pxl-core
//! domain/scene/environment/cloud.rs).

use crate::frame::shader::record::{PIPELINE_CLOUD, PIPELINE_CLOUD_FIELD};
use pxl_gpu_link::enums::{AddressMode, FilterMode, MipmapFilterMode};

use crate::frame::shader::word::clouds::{BASE, CNT, DRAW_DIST, DRIFT, FRAC, SHOWN};
use crate::frame::shader::word::{CLOUD_TILE_TEXELS, CLOUD_TILE_UNITS, CLOUDS_FLOATS};
use crate::gpu::api::{
    Entry, Format, Gpu, Load, TrackedRenderPass, buf_entry, buffer_usage, bytes, pass_desc,
    release, sampler_entry, texture_usage, view_entry, word_sz,
};
use crate::gpu::mip::{MipPass, mip_cnt};
use crate::gpu::pipeline::{Pipeline, PipelineCache};
use crate::pass::atmosphere::AIR_ENTRY_CNT;
use pxl_content::cloud::LATTICE_CNT;
use pxl_content::param::cloud::LATTICE_SHAMT;
use pxl_spec::space::coord::zoom::{EXTENT, UNIT_ZOOM};

const _: () = assert!(
    EXTENT[0] == UNIT_ZOOM && EXTENT[1] == UNIT_ZOOM,
    "one lattice count for both axes"
);
const _: () = assert!(
    LATTICE_CNT.is_multiple_of(CLOUD_TILE_UNITS),
    "the world is whole tiles"
);

pub struct CloudPass {
    pipeline: Pipeline,
    uniform: u32,
    group: u32,
    words: [f32; CLOUDS_FLOATS],
    /// The field's tile, the pipeline that draws it and the pass that builds its mips, held
    /// until `bake` bakes the tile. The renderer calls `bake` once its pipelines are made,
    /// before its first frame.
    unbaked: Option<(u32, Pipeline, MipPass)>,
}

impl CloudPass {
    pub fn new(
        g: &Gpu,
        pipelines: &mut PipelineCache,
        frame_buf: u32,
        air: [Entry; AIR_ENTRY_CNT],
    ) -> CloudPass {
        let pipeline = pipelines.build_world(g, PIPELINE_CLOUD);
        let tile = g.texture(
            CLOUD_TILE_TEXELS,
            CLOUD_TILE_TEXELS,
            1,
            Format::R8unorm,
            mip_cnt(CLOUD_TILE_TEXELS, CLOUD_TILE_TEXELS),
            texture_usage::TEXTURE_BINDING | texture_usage::RENDER_ATTACHMENT,
        );
        let tile_view = g.view(
            tile,
            false,
            0,
            mip_cnt(CLOUD_TILE_TEXELS, CLOUD_TILE_TEXELS),
            0,
            1,
        );
        // The sampler repeats, as the tile is seamless: each edge meets the opposite one.
        let tile_sampler = g.sampler(
            FilterMode::Linear,
            FilterMode::Linear,
            MipmapFilterMode::Linear,
            AddressMode::Repeat,
            None,
            1,
        );
        let uniform = g.buf(
            word_sz(CLOUDS_FLOATS),
            buffer_usage::UNIFORM | buffer_usage::COPY_DST,
        );
        let mut e = vec![
            buf_entry(0, frame_buf),
            buf_entry(1, uniform),
            view_entry(2, tile_view),
            sampler_entry(3, tile_sampler),
        ];
        e.extend(air);
        let group = g.bind_group(pipeline.layouts[0], &e);
        let field = pipelines.build(g, PIPELINE_CLOUD_FIELD, Format::R8unorm);
        let mips = MipPass::new(g, pipelines, Format::R8unorm);
        CloudPass {
            pipeline,
            uniform,
            group,
            words: [0.0; CLOUDS_FLOATS],
            unbaked: Some((tile, field, mips)),
        }
    }

    /// Draws the field's tile and then its mips. Only the first call does anything; the
    /// renderer makes it before its first frame, once the pipelines are made.
    pub fn bake(&mut self, g: &Gpu) {
        let Some((tile, field, mips)) = self.unbaked.take() else {
            return;
        };
        let encoder = g.encoder();
        let level = g.view(tile, false, 0, 1, 0, 1);
        let pass = g.pass(
            encoder,
            &pass_desc(level, Load::Clear, 0, Load::Clear, 0.0, None),
        );
        let mut pass = TrackedRenderPass::new(g, pass);
        pass.set_pipeline(field.id);
        pass.draw(3, 1, 0, 0);
        pass.end();
        release(level);
        g.submit(encoder);
        mips.build(g, tile, mip_cnt(CLOUD_TILE_TEXELS, CLOUD_TILE_TEXELS), 0, 1);
        mips.release();
    }

    /// Lays out the clouds' words. `eye` is the eye's place on the map, in the world's
    /// coordinates as doubles. `drift` is how far the wind has carried the field, and `shown`
    /// is how much of the clouds the core shows. No sheet is drawn past `draw_dist_km`.
    pub fn prepare(&mut self, eye: [f64; 2], drift: [f64; 2], shown: f64, draw_dist_km: f32) {
        self.words[SHOWN] = shown as f32;
        self.words[DRAW_DIST] = draw_dist_km;
        field(&mut self.words, eye, drift);
    }

    /// Writes `words` to the device. The frame calls it only when its read found them
    /// changed.
    pub fn write(&self, g: &Gpu) {
        g.write_buf(self.uniform, 0, bytes(&self.words));
    }

    /// The words the pass reads, as `prepare` wrote them.
    pub fn words(&self) -> &[f32] {
        &self.words
    }

    pub fn draw(&self, pass: &mut TrackedRenderPass) {
        if self.words[SHOWN] <= 0.0 {
            return;
        }
        pass.set_pipeline(self.pipeline.id);
        pass.set_bind_group(0, self.group, &[]);
        // The pass draws one quad for each sheet, the farther sheet first (cloud.wgsl `vs`).
        pass.draw(6, 2, 0, 0);
    }
}

/// Writes the eye's place on the field's lattice, split into a whole lattice unit and a
/// fraction of one, so the shader never holds a coordinate as large as the world. `eye` is in
/// the world's coordinates as doubles. The world is `LATTICE_CNT` units around, so a lap of X
/// is a whole number of units and the whole unit wraps with it. It also writes how far the
/// wind has carried the field.
fn field(words: &mut [f32; CLOUDS_FLOATS], eye: [f64; 2], drift: [f64; 2]) {
    let side = (1u32 << LATTICE_SHAMT) as f64;
    for k in 0..2 {
        let u = eye[k] / side;
        words[BASE + k] =
            f32::from_bits((u.floor() as i64).rem_euclid(i64::from(LATTICE_CNT)) as u32);
        words[FRAC + k] = (u - u.floor()) as f32;
        words[DRIFT + k] = drift[k] as f32;
    }
    words[CNT] = f32::from_bits(LATTICE_CNT);
}
