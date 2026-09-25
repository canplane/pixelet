// pxl-render/gpu/pass/decor.rs

//! Decor on the device: the scheduler's decor draws (draw.rs) executed
//! as instanced draws of each variant's model, the instances read straight out
//! of the decor block the scheduler built (crates/pxl/web/kernel/src/layer/decor/build.rs's layout) as it lies
//! on the GPU: the block is the instance buffer, the chunk's translation is its
//! uniform, and the scheduler says which block a chunk shows.
//!
//! A model is the scheduler's (crates/pxl/web/kernel/src/layer/model/store.rs), registered by the decor asset
//! store and drawn from the same block the kernel holds its triangles in for
//! the hit test — the vertex lanes of crates/pxl/web/kernel/src/layer/model/asset.rs's layout, and the indices
//! past them. The groups are every bundle's variants in bundle order, the order
//! the kinds were taught; a group's skin is the image its variant loaded, held
//! by image rather than by group, since two variants — two kinds, even — may
//! name the same skin and a texture is uploaded per row, not per group.

use crate::frame::draw::{DRAW_WORDS, PIPELINE_DECOR, PIPELINE_GHOST_DECOR, UNIFORM_FLOATS};
use crate::frame::shader::record::{PIPELINE_DECOR_DEPTH, PIPELINE_DECOR_PREPASS};
use crate::gpu::api::{object, release, GpuApi};
use crate::gpu::api_spec::{address, compare, filter, format};
use crate::gpu::atmosphere::AIR_WORDS;
use crate::gpu::pass::model::{draw_model, DrawUniforms, FrameGroups};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::renderer::{put_f32, put_u32, skin_group, Blocks};
use crate::gpu::shadow::Shadow;

/// A decor draw's dynamic uniform (shader/decor.wgsl's Draw): translation, the
/// group's first lane word, the instance count, the opacity, padding.
const DRAW_UNIFORM_BYTES: u32 = 32;
/// Decor groups the pass holds a model and a skin for: every bundle's
/// variants, in bundle order.
pub const GROUPS: usize = 64;

/// No skin row taken yet.
const NO_SKIN: usize = usize::MAX;

#[derive(Clone, Copy)]
struct Group {
    /// The model's mesh block, by the draw list's handle, and its skin's image.
    mesh: u32,
    image: Option<u32>,
    /// The skin row this group draws with.
    skin: usize,
}

impl Default for Group {
    fn default() -> Group {
        Group { mesh: 0, image: None, skin: NO_SKIN }
    }
}

/// One image on the device, shared by every group whose variant names it.
#[derive(Clone, Copy, Default)]
struct Skin {
    image: u32,
    /// Texture, view, group.
    ids: [u32; 3],
    /// Groups drawing with this row; the row goes back when the last leaves.
    uses: u32,
}

fn is_decor(pipeline: u32) -> bool {
    pipeline == PIPELINE_DECOR || pipeline == PIPELINE_GHOST_DECOR
}

pub struct DecorPass {
    lit: Built,
    ghost: Built,
    depth: Built,
    prepass: Built,
    frames: FrameGroups,
    sampler: u32,
    uniforms: DrawUniforms,
    groups: [Group; GROUPS],
    skins: [Skin; GROUPS],
}

impl DecorPass {
    /// Over `frame_buffer`, the shadow map, the occlusion map's entry `ao` and
    /// the air's entries.
    #[allow(clippy::too_many_arguments)]
    pub fn new(g: &GpuApi, pipelines: &mut Pipelines, samples: u32, frame_buffer: u32, shadow: &Shadow, ao: [u32; 5], air: [u32; AIR_WORDS], align: u32) -> DecorPass {
        let lit = pipelines.build(g, PIPELINE_DECOR, format::CANVAS, samples);
        let sampler = object();
        (g.sampler)(sampler, filter::LINEAR, filter::LINEAR, filter::NEAREST, address::REPEAT, compare::NONE, 1);
        DecorPass {
            ghost: pipelines.build(g, PIPELINE_GHOST_DECOR, format::CANVAS, samples),
            depth: pipelines.build(g, PIPELINE_DECOR_DEPTH, format::CANVAS, samples),
            prepass: pipelines.build(g, PIPELINE_DECOR_PREPASS, format::CANVAS, 1),
            frames: FrameGroups::new(g, lit.layouts[0], frame_buffer, shadow.entries(1), Some((shadow.light_frame, shadow.blank_entries(1))), ao, air),
            lit,
            sampler,
            uniforms: DrawUniforms::new(DRAW_UNIFORM_BYTES, align),
            groups: [Group::default(); GROUPS],
            skins: [Skin::default(); GROUPS],
        }
    }

    pub fn set_samples(&mut self, g: &GpuApi, pipelines: &mut Pipelines, samples: u32) {
        self.lit = pipelines.build(g, PIPELINE_DECOR, format::CANVAS, samples);
        self.ghost = pipelines.build(g, PIPELINE_GHOST_DECOR, format::CANVAS, samples);
        self.depth = pipelines.build(g, PIPELINE_DECOR_DEPTH, format::CANVAS, samples);
    }

    /// Group 0 again over a new occlusion map's entry `ao`.
    pub fn rebind(&mut self, g: &GpuApi, frame_buffer: u32, shadow: &Shadow, ao: [u32; 5]) {
        self.frames
            .rebind(g, self.lit.layouts[0], frame_buffer, shadow.entries(1), Some((shadow.light_frame, shadow.blank_entries(1))), ao);
    }

    /// Each group's model block and skin image as the kernel has them now, in
    /// group order (0 for a model not yet registered); a group whose model or
    /// skin changed lets its skin go.
    pub fn set_models(&mut self, groups: &[(u32, Option<u32>)]) {
        for (group, &(mesh, image)) in groups.iter().take(GROUPS).enumerate() {
            let gr = self.groups[group];
            if gr.mesh != mesh || gr.image != image {
                self.groups[group] = Group { mesh, image, skin: NO_SKIN };
                self.release_skin(gr.skin);
            }
        }
    }

    /// A group's model block and counts and its skin's group, the skin made on
    /// first use once its image is here.
    fn model_of(&mut self, g: &GpuApi, group: u32, blocks: &Blocks) -> Option<(u32, u32, u32, u32)> {
        let gr = *self.groups.get(group as usize)?;
        let (buffer, vertex_cnt, index_cnt) = blocks.get(gr.mesh)?;
        let at = if gr.skin == NO_SKIN { self.take_skin(g, gr.image?)? } else { gr.skin };
        self.groups[group as usize].skin = at;
        Some((buffer, vertex_cnt, index_cnt, self.skins[at].ids[2]))
    }

    /// The row `image` is on, made on first use and shared from then on: two
    /// variants naming one skin are one texture on the device.
    fn take_skin(&mut self, g: &GpuApi, image: u32) -> Option<usize> {
        if let Some(at) = self.skins.iter().position(|s| s.uses > 0 && s.image == image) {
            self.skins[at].uses += 1;
            return Some(at);
        }
        let at = self.skins.iter().position(|s| s.uses == 0)?;
        let ids = skin_group(g, self.lit.layouts[2], self.sampler, image)?;
        self.skins[at] = Skin { image, ids, uses: 1 };
        Some(at)
    }

    /// One group less on a row; its texture, view and group go back once the
    /// last group has left it.
    fn release_skin(&mut self, at: usize) {
        let Some(s) = self.skins.get_mut(at) else { return };
        s.uses = s.uses.saturating_sub(1);
        if s.uses == 0 {
            for id in s.ids.into_iter().rev() {
                release(id);
            }
            *s = Skin::default();
        }
    }

    /// Triangles one instance of `group` draws; 0 while it loads.
    pub fn triangles_of(&self, group: u32, blocks: &Blocks) -> u32 {
        self.groups.get(group as usize).and_then(|gr| blocks.get(gr.mesh)).map_or(0, |(_, _, index_cnt)| index_cnt / 3)
    }

    /// Writes every decor draw's uniforms, ghosts included, in one write.
    pub fn prepare(&mut self, g: &GpuApi, draws: &[u32], uniforms: &[f32]) {
        let mine = || draws.as_chunks::<DRAW_WORDS>().0.iter().filter(|d| is_decor(d[4]));
        self.uniforms.write(g, self.lit.layouts[1], mine().count() as u32, mine(), |buf, d| {
            let u = d[3] as usize * UNIFORM_FLOATS;
            put_f32(buf, 0, uniforms[u]);
            put_f32(buf, 4, uniforms[u + 1]);
            put_f32(buf, 8, uniforms[u + 2]);
            put_u32(buf, 12, uniforms[u + 3] as u32);
            put_u32(buf, 16, d[2]);
            put_f32(buf, 20, uniforms[u + 4]);
            put_u32(buf, 24, 0);
            put_u32(buf, 28, 0);
        });
    }

    /// The opaque decor into the open pass, after `prepare`.
    pub fn draw(&mut self, g: &GpuApi, pass: u32, draws: &[u32], blocks: &mut Blocks) -> u32 {
        self.draw_pipeline(g, pass, draws, blocks, PIPELINE_DECOR, self.lit.pipeline, self.frames.frame)
    }

    /// The opaque decor's depth from the light.
    pub fn draw_depth(&mut self, g: &GpuApi, pass: u32, draws: &[u32], blocks: &mut Blocks) -> u32 {
        self.draw_pipeline(g, pass, draws, blocks, PIPELINE_DECOR, self.depth.pipeline, self.frames.light)
    }

    /// The opaque decor's depth from the eye, for the screen-space occlusion.
    pub fn draw_prepass(&mut self, g: &GpuApi, pass: u32, draws: &[u32], blocks: &mut Blocks) -> u32 {
        self.draw_pipeline(g, pass, draws, blocks, PIPELINE_DECOR, self.prepass.pipeline, self.frames.frame)
    }

    /// The ghost's decor, blended, after everything opaque.
    pub fn draw_ghosts(&mut self, g: &GpuApi, pass: u32, draws: &[u32], blocks: &mut Blocks) -> u32 {
        self.draw_pipeline(g, pass, draws, blocks, PIPELINE_GHOST_DECOR, self.ghost.pipeline, self.frames.frame)
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_pipeline(&mut self, g: &GpuApi, pass: u32, draws: &[u32], blocks: &mut Blocks, which: u32, pipeline: u32, group0: u32) -> u32 {
        let mut drawn = 0;
        for (k, d) in draws.as_chunks::<DRAW_WORDS>().0.iter().filter(|d| is_decor(d[4])).enumerate() {
            let Some((group, offset)) = self.uniforms.at(k as u32) else { break };
            if d[4] != which {
                continue;
            }
            let Some((model, vertex_cnt, index_cnt, skin)) = self.model_of(g, d[1], blocks) else { continue };
            let Some(block) = blocks.group(g, d[0], self.lit.layouts[3]) else { continue };
            if drawn == 0 {
                (g.set_pipeline)(pass, pipeline);
                (g.set_bind_group)(pass, 0, group0, &[]);
            }
            (g.set_bind_group)(pass, 1, group, &[offset]);
            (g.set_bind_group)(pass, 2, skin, &[]);
            (g.set_bind_group)(pass, 3, block, &[]);
            draw_model(g, pass, &self.lit, model, vertex_cnt, index_cnt, d[2]);
            drawn += 1;
        }
        drawn
    }
}
