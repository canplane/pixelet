// pxl-render/gpu/pass/decor.rs

//! Decor as instanced draws of each variant's model; the decor block is the
//! instance buffer and its index list says which instances show. Skins are held
//! by image, since several variants may name the same one.

use crate::frame::draw::{PIPELINE_DECOR, PIPELINE_GHOST_DECOR};
use crate::frame::shader::record::{PIPELINE_DECOR_DEPTH, PIPELINE_DECOR_PREPASS};
use crate::gpu::api::{object, Gpu};
use crate::gpu::api_spec::{address, compare, filter, format};
use crate::gpu::pass::impostor::Extent;
use crate::gpu::pass::model::{draw_models, put_instances, uniform_of, DrawUniforms, ModelDraw, Stage};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::renderer::{release_skin, skin_group, Blocks, Draws};

/// Translation, first lane word, stride, opacity, padding, index list's first word.
const DRAW_UNIFORM_BYTES: u32 = 32;
pub const GROUPS: usize = 64;

const NO_SKIN: usize = usize::MAX;

#[derive(Clone, Copy)]
struct Group {
    mesh: u32,
    image: Option<u32>,
    skin: usize,
}

impl Default for Group {
    fn default() -> Group {
        Group { mesh: 0, image: None, skin: NO_SKIN }
    }
}

#[derive(Clone, Copy, Default)]
struct Skin {
    image: u32,
    ids: [u32; 3],
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

    /// 0 for a model not yet registered; a group whose model or skin changed lets its skin go.
    pub fn set_models(&mut self, groups: &[(u32, Option<u32>, Extent)]) {
        for (group, &(mesh, image, _)) in groups.iter().take(GROUPS).enumerate() {
            let gr = self.groups[group];
            if gr.mesh != mesh || gr.image != image {
                self.groups[group] = Group { mesh, image, skin: NO_SKIN };
                self.release_skin(gr.skin);
            }
        }
    }

    pub(crate) fn model_of(&mut self, g: &Gpu, group: u32, blocks: &Blocks) -> Option<(u32, u32, u32, u32)> {
        let gr = *self.groups.get(group as usize)?;
        let (buffer, vertex_cnt, index_cnt) = blocks.get(gr.mesh)?;
        let at = if gr.skin == NO_SKIN { self.take_skin(g, gr.image?)? } else { gr.skin };
        self.groups[group as usize].skin = at;
        Some((buffer, vertex_cnt, index_cnt, self.skins[at].ids[2]))
    }

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

    fn release_skin(&mut self, at: usize) {
        let Some(s) = self.skins.get_mut(at) else { return };
        s.uses = s.uses.saturating_sub(1);
        if s.uses == 0 {
            release_skin(s.ids);
            *s = Skin::default();
        }
    }

    /// 0 while it loads.
    pub fn triangles_of(&self, group: u32, blocks: &Blocks) -> u32 {
        self.groups.get(group as usize).and_then(|gr| blocks.get(gr.mesh)).map_or(0, |(_, _, index_cnt)| index_cnt / 3)
    }

    pub fn prepare(&mut self, g: &Gpu, draws: &Draws, uniforms: &[f32]) {
        let (opaque, ghost) = (draws.of(PIPELINE_DECOR), draws.of(PIPELINE_GHOST_DECOR));
        let count = (opaque.len() + ghost.len()) as u32;
        self.uniforms.write(g, self.lit.layouts[1], count, opaque.chain(ghost), |buf, d| {
            put_instances(buf, uniform_of(uniforms, d), 0);
        });
    }

    pub fn draw(&mut self, g: &Gpu, pass: u32, draws: &Draws, blocks: &mut Blocks, stage: Stage, frame: (u32, &mut u32)) {
        let pipeline = match stage {
            Stage::Lit => self.lit.pipeline,
            Stage::Depth => self.depth.pipeline,
            Stage::Prepass => self.prepass.pipeline,
        };
        self.draw_pipeline(g, pass, draws, blocks, PIPELINE_DECOR, pipeline, frame);
    }

    pub fn draw_ghosts(&mut self, g: &Gpu, pass: u32, draws: &Draws, blocks: &mut Blocks, frame: (u32, &mut u32)) {
        self.draw_pipeline(g, pass, draws, blocks, PIPELINE_GHOST_DECOR, self.ghost.pipeline, frame);
    }

    /// The ghost's uniforms follow the opaque ones' (`prepare`).
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
