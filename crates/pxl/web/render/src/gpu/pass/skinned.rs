// pxl-render/gpu/pass/skinned.rs

//! Actors on the device: the scheduler's skinned draws (crates/pxl/web/kernel/src/io/load.rs)
//! executed against each model's mesh block and each actor's joint palette,
//! both as the scheduler wrote them, under shader/skinned.wgsl. A model's skin
//! is the image its asset loaded, by model id.
//!
//! The world's pass lights them under the sun's shadow; a case of the variant
//! picker draws its one actor through a pass of its own, at one sample, with a
//! blank map where the shadow would be.

use crate::frame::draw::{DRAW_WORDS, PIPELINE_SKINNED, UNIFORM_FLOATS};
use crate::frame::shader::record::{PIPELINE_SKINNED_DEPTH, PIPELINE_SKINNED_PREPASS};
use crate::gpu::api::{object, release, GpuApi};
use crate::gpu::api_spec::{address, compare, filter, format};
use crate::gpu::atmosphere::AIR_WORDS;
use crate::gpu::pass::model::{draw_model, DrawUniforms, FrameGroups};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::renderer::{put_f32, skin_group, Blocks};

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
    groups: FrameGroups,
    sampler: u32,
    uniforms: DrawUniforms,
    skins: [Skin; SKINS],
    /// Counts `set_skin` calls, to stamp the slot each one lands in.
    seq: u32,
}

impl SkinnedPass {
    /// Over `frame_buffer`, a shadow map's entries, the occlusion map's entry
    /// `ao` and the air's entries; `light` the light's frame and its entries
    /// where the pass draws into the shadow map too.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        g: &GpuApi,
        pipelines: &mut Pipelines,
        samples: u32,
        frame_buffer: u32,
        shadow: [u32; 10],
        light: Option<(u32, [u32; 10])>,
        ao: [u32; 5],
        air: [u32; AIR_WORDS],
        align: u32,
    ) -> SkinnedPass {
        let lit = pipelines.build(g, PIPELINE_SKINNED, format::CANVAS, samples);
        let sampler = object();
        (g.sampler)(sampler, filter::LINEAR, filter::LINEAR, filter::NEAREST, address::REPEAT, compare::NONE, 1);
        SkinnedPass {
            depth: pipelines.build(g, PIPELINE_SKINNED_DEPTH, format::CANVAS, samples),
            prepass: pipelines.build(g, PIPELINE_SKINNED_PREPASS, format::CANVAS, 1),
            groups: FrameGroups::new(g, lit.layouts[0], frame_buffer, shadow, light, ao, air),
            lit,
            sampler,
            uniforms: DrawUniforms::new(DRAW_UNIFORM_BYTES, align),
            skins: [Skin::default(); SKINS],
            seq: 0,
        }
    }

    pub fn set_samples(&mut self, g: &GpuApi, pipelines: &mut Pipelines, samples: u32) {
        self.lit = pipelines.build(g, PIPELINE_SKINNED, format::CANVAS, samples);
        self.depth = pipelines.build(g, PIPELINE_SKINNED_DEPTH, format::CANVAS, samples);
    }

    /// Group 0 again over a new occlusion map's entry `ao`.
    pub fn rebind(&mut self, g: &GpuApi, frame_buffer: u32, shadow: [u32; 10], light: Option<(u32, [u32; 10])>, ao: [u32; 5]) {
        self.groups.rebind(g, self.lit.layouts[0], frame_buffer, shadow, light, ao);
    }

    /// Model `model`'s skin; the same image again changes nothing, and one not
    /// yet arrived is asked for again next frame.
    pub fn set_skin(&mut self, g: &GpuApi, model: u32, image: u32) {
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
        for id in s.ids.into_iter().rev() {
            release(id);
        }
        *s = Skin::default();
        if let Some(ids) = skin_group(g, self.lit.layouts[2], self.sampler, image) {
            *s = Skin { model, image, ids, seq };
        }
    }

    fn skin_of(&self, model: u32) -> Option<u32> {
        self.skins.iter().find(|s| s.model == model && s.ids[2] != 0).map(|s| s.ids[2])
    }

    /// Writes every skinned draw's uniforms in one write.
    pub fn prepare(&mut self, g: &GpuApi, draws: &[u32], uniforms: &[f32]) {
        let mine = || draws.as_chunks::<DRAW_WORDS>().0.iter().filter(|d| d[4] == PIPELINE_SKINNED);
        self.uniforms.write(g, self.lit.layouts[1], mine().count() as u32, mine(), |buf, d| {
            let u = d[3] as usize * UNIFORM_FLOATS;
            for i in 0..8 {
                put_f32(buf, i * 4, if i < 6 { uniforms[u + i] } else { 0.0 });
            }
        });
    }

    pub fn draw(&mut self, g: &GpuApi, pass: u32, draws: &[u32], blocks: &mut Blocks) -> u32 {
        self.draw_pipeline(g, pass, draws, blocks, self.lit.pipeline, self.groups.frame)
    }

    /// The actors' depth from the light; nothing for a pass with no light.
    pub fn draw_depth(&mut self, g: &GpuApi, pass: u32, draws: &[u32], blocks: &mut Blocks) -> u32 {
        if self.groups.light == 0 {
            return 0;
        }
        self.draw_pipeline(g, pass, draws, blocks, self.depth.pipeline, self.groups.light)
    }

    /// The actors' depth from the eye, for the screen-space occlusion.
    pub fn draw_prepass(&mut self, g: &GpuApi, pass: u32, draws: &[u32], blocks: &mut Blocks) -> u32 {
        self.draw_pipeline(g, pass, draws, blocks, self.prepass.pipeline, self.groups.frame)
    }

    fn draw_pipeline(&mut self, g: &GpuApi, pass: u32, draws: &[u32], blocks: &mut Blocks, pipeline: u32, group0: u32) -> u32 {
        let mut drawn = 0;
        for (k, d) in draws.as_chunks::<DRAW_WORDS>().0.iter().filter(|d| d[4] == PIPELINE_SKINNED).enumerate() {
            let Some((group, offset)) = self.uniforms.at(k as u32) else { break };
            let (Some((mesh, vertex_cnt, index_cnt)), Some(skin)) = (blocks.get(d[0]), self.skin_of(d[2])) else {
                continue;
            };
            let Some(palette) = blocks.group(g, d[1], self.lit.layouts[3]) else { continue };
            if drawn == 0 {
                (g.set_pipeline)(pass, pipeline);
                (g.set_bind_group)(pass, 0, group0, &[]);
            }
            (g.set_bind_group)(pass, 1, group, &[offset]);
            (g.set_bind_group)(pass, 2, skin, &[]);
            (g.set_bind_group)(pass, 3, palette, &[]);
            draw_model(g, pass, &self.lit, mesh, vertex_cnt, index_cnt, 1);
            drawn += 1;
        }
        drawn
    }

    /// Lets go of what the pass made on the device.
    pub fn release(mut self) {
        for s in self.skins {
            for id in s.ids.into_iter().rev() {
                release(id);
            }
        }
        self.uniforms.release();
        self.groups.release();
        release(self.sampler);
    }
}
