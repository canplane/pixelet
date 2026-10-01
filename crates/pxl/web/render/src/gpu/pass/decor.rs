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

use crate::frame::draw::{PIPELINE_DECOR, PIPELINE_GHOST_DECOR};
use crate::frame::shader::record::{PIPELINE_DECOR_DEPTH, PIPELINE_DECOR_PREPASS};
use crate::gpu::api::{object, Gpu};
use crate::gpu::api_spec::{address, compare, filter, format};
use crate::gpu::pass::impostor::Extent;
use crate::gpu::pass::model::{draw_models, put_instances, uniform_of, DrawUniforms, ModelDraw, Stage};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::renderer::{release_skin, skin_group, Blocks, Draws};

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

pub struct DecorPass {
    lit: Built,
    ghost: Built,
    depth: Built,
    prepass: Built,
    sampler: u32,
    uniforms: DrawUniforms,
    groups: [Group; GROUPS],
    skins: [Skin; GROUPS],
}

impl DecorPass {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines, align: u32) -> DecorPass {
        let sampler = object();
        g.sampler(sampler, filter::LINEAR, filter::LINEAR, filter::NEAREST, address::REPEAT, compare::NONE, 1);
        DecorPass {
            lit: pipelines.build(g, PIPELINE_DECOR, format::CANVAS),
            ghost: pipelines.build(g, PIPELINE_GHOST_DECOR, format::CANVAS),
            depth: pipelines.build(g, PIPELINE_DECOR_DEPTH, format::CANVAS),
            prepass: pipelines.build(g, PIPELINE_DECOR_PREPASS, format::CANVAS),
            sampler,
            uniforms: DrawUniforms::new(DRAW_UNIFORM_BYTES, align),
            groups: [Group::default(); GROUPS],
            skins: [Skin::default(); GROUPS],
        }
    }

    /// Each group's model block and skin image as the kernel has them now, in
    /// group order (0 for a model not yet registered); a group whose model or
    /// skin changed lets its skin go.
    pub fn set_models(&mut self, groups: &[(u32, Option<u32>, Extent)]) {
        for (group, &(mesh, image, _)) in groups.iter().take(GROUPS).enumerate() {
            let gr = self.groups[group];
            if gr.mesh != mesh || gr.image != image {
                self.groups[group] = Group { mesh, image, skin: NO_SKIN };
                self.release_skin(gr.skin);
            }
        }
    }

    /// A group's model block and counts and its skin's group, the skin made on
    /// first use once its image is here.
    pub(crate) fn model_of(&mut self, g: &Gpu, group: u32, blocks: &Blocks) -> Option<(u32, u32, u32, u32)> {
        let gr = *self.groups.get(group as usize)?;
        let (buffer, vertex_cnt, index_cnt) = blocks.get(gr.mesh)?;
        let at = if gr.skin == NO_SKIN { self.take_skin(g, gr.image?)? } else { gr.skin };
        self.groups[group as usize].skin = at;
        Some((buffer, vertex_cnt, index_cnt, self.skins[at].ids[2]))
    }

    /// The row `image` is on, made on first use and shared from then on: two
    /// variants naming one skin are one texture on the device.
    fn take_skin(&mut self, g: &Gpu, image: u32) -> Option<usize> {
        if let Some(at) = self.skins.iter().position(|s| s.uses > 0 && s.image == image) {
            self.skins[at].uses += 1;
            return Some(at);
        }
        let at = self.skins.iter().position(|s| s.uses == 0)?;
        let ids = skin_group(g, self.lit.layouts[2], self.sampler, image);
        self.skins[at] = Skin { image, ids, uses: 1 };
        Some(at)
    }

    /// One group less on a row; its texture, view and group go back once the
    /// last group has left it.
    fn release_skin(&mut self, at: usize) {
        let Some(s) = self.skins.get_mut(at) else { return };
        s.uses = s.uses.saturating_sub(1);
        if s.uses == 0 {
            release_skin(s.ids);
            *s = Skin::default();
        }
    }

    /// Triangles one instance of `group` draws; 0 while it loads.
    pub fn triangles_of(&self, group: u32, blocks: &Blocks) -> u32 {
        self.groups.get(group as usize).and_then(|gr| blocks.get(gr.mesh)).map_or(0, |(_, _, index_cnt)| index_cnt / 3)
    }

    /// Writes every decor draw's uniforms in one write, the opaque ones' and
    /// then the ghost's.
    pub fn prepare(&mut self, g: &Gpu, draws: &Draws, uniforms: &[f32]) {
        let (opaque, ghost) = (draws.of(PIPELINE_DECOR), draws.of(PIPELINE_GHOST_DECOR));
        let count = (opaque.len() + ghost.len()) as u32;
        self.uniforms.write(g, self.lit.layouts[1], count, opaque.chain(ghost), |buf, d| {
            put_instances(buf, uniform_of(uniforms, d), 0);
        });
    }

    /// The opaque decor into the open pass at `stage`, after `prepare`, under
    /// group 0 `frame`; `bound` is the group 0 the pass has.
    pub fn draw(&mut self, g: &Gpu, pass: u32, draws: &Draws, blocks: &mut Blocks, stage: Stage, frame: (u32, &mut u32)) {
        let pipeline = match stage {
            Stage::Lit => self.lit.pipeline,
            Stage::Depth => self.depth.pipeline,
            Stage::Prepass => self.prepass.pipeline,
        };
        self.draw_pipeline(g, pass, draws, blocks, PIPELINE_DECOR, pipeline, frame);
    }

    /// The ghost's decor, blended, after everything opaque.
    pub fn draw_ghosts(&mut self, g: &Gpu, pass: u32, draws: &Draws, blocks: &mut Blocks, frame: (u32, &mut u32)) {
        self.draw_pipeline(g, pass, draws, blocks, PIPELINE_GHOST_DECOR, self.ghost.pipeline, frame);
    }

    /// The draws of `which` under `pipeline`: the ghost's uniforms follow the
    /// opaque ones' (`prepare`).
    #[allow(clippy::too_many_arguments)]
    fn draw_pipeline(&mut self, g: &Gpu, pass: u32, draws: &Draws, blocks: &mut Blocks, which: u32, pipeline: u32, frame: (u32, &mut u32)) {
        let Some(slots) = self.uniforms.slots() else { return };
        let first = if which == PIPELINE_GHOST_DECOR { draws.of(PIPELINE_DECOR).len() as u32 } else { 0 };
        let (lit, block_layout) = (self.lit, self.lit.layouts[3]);
        draw_models(g, pass, &lit, pipeline, frame, slots, first, draws.of(which), |d| {
            let (model, vertex_cnt, index_cnt, skin) = self.model_of(g, d[1], blocks)?;
            let block = blocks.group(g, d[0], block_layout)?;
            Some(ModelDraw {
                groups: [skin, block],
                mesh: Some((model, vertex_cnt, index_cnt)),
                instances: d[2],
            })
        });
    }
}
