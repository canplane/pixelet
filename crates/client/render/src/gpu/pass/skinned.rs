// pxl-render/gpu/pass/skinned.rs

//! Actors: each model's mesh block skinned by each actor's joint palette. A
//! case of the variant picker draws one actor in a pass of its own.

use crate::frame::draw::uniform_of;
use crate::frame::draw::{DRAW_WORDS, PIPELINE_SKINNED};
use crate::frame::shader::record::PIPELINE_SKINNED_PREPASS;
use xpute_gpu::enums::{AddressMode, FilterMode, MipmapFilterMode};

use crate::gpu::api::{bytes, release, Gpu};

use crate::gpu::pass::model::{draw_models, DrawUniforms, ModelDraw, Stage};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::renderer::{skin_group, Blocks};

/// Translation, turn about Z, opacity, scale, padding.
const DRAW_UNIFORM_BYTES: u32 = 32;
const SKINS: usize = 16;

#[derive(Clone, Copy, Default)]
struct Skin {
    model: u32,
    image: u32,
    group: u32,
    seq: u32,
}

pub struct SkinnedPass {
    lit: Built,
    prepass: Built,
    sampler: u32,
    uniforms: DrawUniforms,
    skins: [Skin; SKINS],
    seq: u32,
}

impl SkinnedPass {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines, align: u32) -> SkinnedPass {
        let sampler = g.sampler(FilterMode::linear, FilterMode::linear, MipmapFilterMode::nearest, AddressMode::repeat, None, 1);
        SkinnedPass {
            lit: pipelines.build(g, PIPELINE_SKINNED, g.canvas()),
            prepass: pipelines.build(g, PIPELINE_SKINNED_PREPASS, g.canvas()),
            sampler,
            uniforms: DrawUniforms::new(DRAW_UNIFORM_BYTES, align),
            skins: [Skin::default(); SKINS],
            seq: 0,
        }
    }

    /// One not yet arrived is asked for again next frame.
    pub fn set_skin(&mut self, g: &Gpu, model: u32, image: u32) {
        let at = self
            .skins
            .iter()
            .position(|s| s.model == model && s.group != 0)
            .or_else(|| self.skins.iter().position(|s| s.group == 0))
            // Nothing says a model is gone, so a full table would leave new models
            // untextured for good (the character preview walks more variants than slots).
            // The least recently asked slot is taken.
            .unwrap_or_else(|| self.skins.iter().enumerate().min_by_key(|(_, s)| s.seq).map_or(0, |(i, _)| i));
        self.seq += 1;
        let seq = self.seq;
        let s = &mut self.skins[at];
        if s.model == model && s.image == image && s.group != 0 {
            s.seq = seq;
            return;
        }
        release(s.group);
        let group = skin_group(g, self.lit.layouts[2], self.sampler, image);
        *s = Skin { model, image, group, seq };
    }

    fn skin_of(&self, model: u32) -> Option<u32> {
        self.skins.iter().find(|s| s.model == model && s.group != 0).map(|s| s.group)
    }

    /// Every palette group the draws will bind is made here, so drawing makes
    /// nothing that outlives its frame.
    pub fn prepare<'a>(&mut self, g: &Gpu, draws: impl ExactSizeIterator<Item = &'a [u32; DRAW_WORDS]> + Clone, uniforms: &[f32], blocks: &mut Blocks) {
        self.uniforms.write(g, self.lit.layouts[1], draws.len() as u32, draws.clone(), |buf, d| {
            let u = uniform_of(uniforms, d);
            buf[..24].copy_from_slice(bytes(&u[..6]));
            buf[24..32].fill(0);
        });
        for d in draws {
            blocks.group(g, d[1], self.lit.layouts[3]);
        }
    }

    pub fn draw<'a>(&self, g: &Gpu, pass: u32, draws: impl Iterator<Item = &'a [u32; DRAW_WORDS]>, blocks: &mut Blocks, stage: Stage, frame: (u32, &mut u32)) {
        let Some(slots) = self.uniforms.slots() else { return };
        let pipeline = match stage {
            Stage::Lit => self.lit.pipeline,
            Stage::Prepass => self.prepass.pipeline,
        };
        let palette_layout = self.lit.layouts[3];
        draw_models(g, pass, &self.lit, pipeline, frame, slots, 0, draws, |d| {
            let (mesh, vertex_cnt, index_cnt) = blocks.get(d[0])?;
            let skin = self.skin_of(d[2])?;
            Some(ModelDraw {
                groups: [skin, blocks.group(g, d[1], palette_layout)?],
                mesh: Some((mesh, vertex_cnt, index_cnt)),
                instances: 1,
            })
        });
    }

    pub fn release(mut self) {
        for s in self.skins {
            release(s.group);
        }
        self.uniforms.release();
        release(self.sampler);
    }
}
