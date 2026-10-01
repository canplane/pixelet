// pxl-render/gpu/pass/skinned.rs

//! Actors on the device: the scheduler's skinned draws (crates/pxl/web/kernel/src/layer/model/store.rs)
//! executed against each model's mesh block and each actor's joint palette,
//! both as the scheduler wrote them, under shader/skinned.wgsl. A model's skin
//! is the image its asset loaded, by model id.
//!
//! The world's pass lights them under the sun's shadow; a case of the variant
//! picker draws its one actor through a pass of its own, at one sample, with a
//! blank map where the shadow would be.

use crate::frame::draw::{DRAW_WORDS, PIPELINE_SKINNED};
use crate::frame::shader::record::{PIPELINE_SKINNED_DEPTH, PIPELINE_SKINNED_PREPASS};
use crate::gpu::api::{bytes, object, release, Gpu};
use crate::gpu::api_spec::{address, compare, filter, format};
use crate::gpu::pass::model::{draw_models, uniform_of, DrawUniforms, ModelDraw, Stage};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::renderer::{release_skin, skin_group, Blocks};

/// shader/skinned.wgsl's Draw: translation, the turn about Z, the opacity, the
/// scale, padding.
const DRAW_UNIFORM_BYTES: u32 = 32;
const SKINS: usize = 16;

#[derive(Clone, Copy, Default)]
struct Skin {
    model: u32,
    image: u32,
    /// Texture, view, group.
    ids: [u32; 3],
    /// When the slot was last asked for, so the one longest unasked is the one
    /// a new model takes.
    seq: u32,
}

pub struct SkinnedPass {
    lit: Built,
    depth: Built,
    prepass: Built,
    sampler: u32,
    uniforms: DrawUniforms,
    skins: [Skin; SKINS],
    /// Counts `set_skin` calls, to stamp the slot each one lands in.
    seq: u32,
}

impl SkinnedPass {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines, align: u32) -> SkinnedPass {
        let sampler = object();
        g.sampler(sampler, filter::LINEAR, filter::LINEAR, filter::NEAREST, address::REPEAT, compare::NONE, 1);
        SkinnedPass {
            lit: pipelines.build(g, PIPELINE_SKINNED, format::CANVAS),
            depth: pipelines.build(g, PIPELINE_SKINNED_DEPTH, format::CANVAS),
            prepass: pipelines.build(g, PIPELINE_SKINNED_PREPASS, format::CANVAS),
            sampler,
            uniforms: DrawUniforms::new(DRAW_UNIFORM_BYTES, align),
            skins: [Skin::default(); SKINS],
            seq: 0,
        }
    }

    /// Model `model`'s skin; the same image again changes nothing, and one not
    /// yet arrived is asked for again next frame.
    pub fn set_skin(&mut self, g: &Gpu, model: u32, image: u32) {
        let at = self
            .skins
            .iter()
            .position(|s| s.model == model && s.ids[2] != 0)
            .or_else(|| self.skins.iter().position(|s| s.ids[2] == 0))
            // Nothing tells the pass a model is gone, so a slot is never freed
            // on its own: with the table full a new model would go untextured
            // for good. The character preview walks every shipped variant,
            // more of them than there are slots. The slot asked for longest
            // ago is the one not drawn.
            .unwrap_or_else(|| self.skins.iter().enumerate().min_by_key(|(_, s)| s.seq).map_or(0, |(i, _)| i));
        self.seq += 1;
        let seq = self.seq;
        let s = &mut self.skins[at];
        if s.model == model && s.image == image && s.ids[2] != 0 {
            s.seq = seq;
            return;
        }
        release_skin(s.ids);
        let ids = skin_group(g, self.lit.layouts[2], self.sampler, image);
        *s = Skin { model, image, ids, seq };
    }

    fn skin_of(&self, model: u32) -> Option<u32> {
        self.skins.iter().find(|s| s.model == model && s.ids[2] != 0).map(|s| s.ids[2])
    }

    /// Writes every skinned draw's uniforms in one write: the first six of
    /// the draw's floats, padded.
    pub fn prepare<'a>(&mut self, g: &Gpu, draws: impl ExactSizeIterator<Item = &'a [u32; DRAW_WORDS]>, uniforms: &[f32]) {
        self.uniforms.write(g, self.lit.layouts[1], draws.len() as u32, draws, |buf, d| {
            let u = uniform_of(uniforms, d);
            buf[..24].copy_from_slice(bytes(&u[..6]));
            buf[24..32].fill(0);
        });
    }

    /// The actors into the open pass at `stage`, after `prepare`, under group
    /// 0 `frame`; `bound` is the group 0 the pass has.
    pub fn draw<'a>(&self, g: &Gpu, pass: u32, draws: impl Iterator<Item = &'a [u32; DRAW_WORDS]>, blocks: &mut Blocks, stage: Stage, frame: (u32, &mut u32)) {
        let Some(slots) = self.uniforms.slots() else { return };
        let pipeline = match stage {
            Stage::Lit => self.lit.pipeline,
            Stage::Depth => self.depth.pipeline,
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

    /// Lets go of what the pass made on the device.
    pub fn release(mut self) {
        for s in self.skins {
            release_skin(s.ids);
        }
        self.uniforms.release();
        release(self.sampler);
    }
}
