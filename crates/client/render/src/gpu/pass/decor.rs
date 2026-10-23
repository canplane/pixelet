// pxl-render/gpu/pass/decor.rs

//! Decor as instanced draws of each variant's model; the decor block is the
//! instance buffer and its index list says which instances show. Skins are held
//! by image, since several variants may name the same one.

use crate::frame::draw::uniform_of;
use crate::frame::draw::{PIPELINE_DECOR, PIPELINE_GHOST_DECOR};
use crate::frame::shader::record::PIPELINE_DECOR_PREPASS;
use crate::gpu::renderer::Image;
use xpute_gpu::enums::{AddressMode, FilterMode, MipmapFilterMode};

use crate::gpu::api::{buffer_entry, buffer_range_entry, release, Gpu};
use crate::gpu::grown::Drawn;
use crate::gpu::pass::impostor::Extent;
use crate::gpu::pass::model::{bind_mesh, draw_models, put_instances, DrawUniforms, ModelDraw, Stage, DRAW_BYTES};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::renderer::{skin_group, Blocks, Draws};

pub const GROUPS: usize = 64;

const NO_SKIN: usize = usize::MAX;

#[derive(Clone, Copy)]
struct Group {
    mesh: u32,
    image: Option<Image>,
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
    group: u32,
    uses: u32,
}

pub struct DecorPass {
    lit: Built,
    ghost: Built,
    prepass: Built,
    sampler: u32,
    uniforms: DrawUniforms,
    groups: [Group; GROUPS],
    skins: [Skin; GROUPS],
    /// The GPU's decor's bind groups (gpu/grown.rs), each with the buffer it
    /// binds: its regions' draw uniforms and its block.
    grown_uniforms: (u32, u32),
    grown_block: (u32, u32),
}

impl DecorPass {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines, align: u32) -> DecorPass {
        let sampler = g.sampler(FilterMode::linear, FilterMode::linear, MipmapFilterMode::nearest, AddressMode::repeat, None, 1);
        DecorPass {
            lit: pipelines.build(g, PIPELINE_DECOR, g.canvas()),
            ghost: pipelines.build(g, PIPELINE_GHOST_DECOR, g.canvas()),
            prepass: pipelines.build(g, PIPELINE_DECOR_PREPASS, g.canvas()),
            sampler,
            uniforms: DrawUniforms::new(DRAW_BYTES, align),
            groups: [Group::default(); GROUPS],
            skins: [Skin::default(); GROUPS],
            grown_uniforms: (0, 0),
            grown_block: (0, 0),
        }
    }

    /// 0 for a model not yet registered; a group whose model or skin changed lets its skin go.
    pub fn set_models(&mut self, groups: &[(u32, Option<Image>, Extent)]) {
        for (group, &(mesh, image, _)) in groups.iter().take(GROUPS).enumerate() {
            let gr = self.groups[group];
            if gr.mesh != mesh || gr.image.map(|i| i.0) != image.map(|i| i.0) {
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
        Some((buffer, vertex_cnt, index_cnt, self.skins[at].group))
    }

    /// None while its image lands.
    fn take_skin(&mut self, g: &Gpu, image: Image) -> Option<usize> {
        if let Some(at) = self.skins.iter().position(|s| s.uses > 0 && s.image == image.0) {
            self.skins[at].uses += 1;
            return Some(at);
        }
        let at = self.skins.iter().position(|s| s.uses == 0)?;
        let group = skin_group(g, self.lit.layouts[2], self.sampler, image)?;
        self.skins[at] = Skin { image: image.0, group, uses: 1 };
        Some(at)
    }

    fn release_skin(&mut self, at: usize) {
        let Some(s) = self.skins.get_mut(at) else { return };
        s.uses = s.uses.saturating_sub(1);
        if s.uses == 0 {
            release(s.group);
            *s = Skin::default();
        }
    }

    /// 0 while it loads.
    pub fn triangles_of(&self, group: u32, blocks: &Blocks) -> u32 {
        self.groups.get(group as usize).and_then(|gr| blocks.get(gr.mesh)).map_or(0, |(_, _, index_cnt)| index_cnt / 3)
    }

    /// Every skin and block group the draws will bind is made here, so drawing
    /// makes nothing that outlives its frame.
    /// `moved` whether the lists the draws are made from moved.
    pub fn prepare(&mut self, g: &Gpu, draws: &Draws, uniforms: &[f32], moved: bool, blocks: &mut Blocks) {
        let (opaque, ghost) = (draws.of(PIPELINE_DECOR), draws.of(PIPELINE_GHOST_DECOR));
        let count = (opaque.len() + ghost.len()) as u32;
        self.uniforms.write(g, self.lit.layouts[1], count, opaque.clone().chain(ghost.clone()), moved, |buf, d| {
            put_instances(buf, uniform_of(uniforms, d), 0);
        });
        for d in opaque.chain(ghost) {
            self.model_of(g, d[1], blocks);
            blocks.group(g, d[0], self.lit.layouts[3]);
        }
    }

    pub fn draw(&mut self, g: &Gpu, pass: u32, draws: &Draws, blocks: &mut Blocks, stage: Stage, frame: (u32, &mut u32)) {
        let pipeline = match stage {
            Stage::Lit => self.lit.pipeline,
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

    /// The GPU's decor's bind groups, made again only where its buffers were.
    pub fn prepare_grown(&mut self, g: &Gpu, grown: Option<Drawn>) {
        let Some(d) = grown else { return };
        self.grown_uniforms = uniform_group(g, self.grown_uniforms, d.uniforms, self.lit.layouts[1]);
        self.grown_block = block_group(g, self.grown_block, d.block, self.lit.layouts[3]);
    }

    /// The models the GPU grew, an indirect draw a group whose model is held
    /// (pxl-portable `scatter::drawn`, a group's models its region `2·group`).
    pub fn draw_grown(&mut self, g: &Gpu, pass: u32, grown: Option<Drawn>, blocks: &Blocks, stage: Stage, (frame, bound): (u32, &mut u32)) {
        let Some(d) = grown.filter(|_| self.grown_block.1 != 0) else { return };
        let pipeline = match stage {
            Stage::Lit => self.lit.pipeline,
            Stage::Prepass => self.prepass.pipeline,
        };
        g.set_pipeline(pass, pipeline);
        if *bound != frame {
            g.set_bind_group(pass, 0, frame, &[]);
            *bound = frame;
        }
        g.set_bind_group(pass, 3, self.grown_block.1, &[]);
        let mut mesh_bound = 0;
        for group in 0..pxl_portable::scatter::GROUPS {
            let Some((model, vertex_cnt, index_cnt, skin)) = self.model_of(g, group, blocks) else { continue };
            g.set_bind_group(pass, 1, self.grown_uniforms.1, &[d.uniform_at(2 * group)]);
            g.set_bind_group(pass, 2, skin, &[]);
            bind_mesh(g, pass, &self.lit, &mut mesh_bound, model, vertex_cnt, index_cnt);
            g.draw_indexed_indirect(pass, d.args, d.args_at(2 * group));
        }
    }
}

/// A bind group over the GPU's decor block `block` in `layout`, `held` kept
/// while it binds that block.
pub(crate) fn block_group(g: &Gpu, held: (u32, u32), block: u32, layout: u32) -> (u32, u32) {
    if held.0 == block {
        return held;
    }
    release(held.1);
    let id = g.bind_group(layout, &[buffer_entry(0, block)]);
    (block, id)
}

/// A bind group over the GPU's decor's draw uniforms `uniforms` in `layout`,
/// a draw's uniform at a dynamic offset, `held` kept while it binds them.
pub(crate) fn uniform_group(g: &Gpu, held: (u32, u32), uniforms: u32, layout: u32) -> (u32, u32) {
    if held.0 == uniforms {
        return held;
    }
    release(held.1);
    let id = g.bind_group(layout, &[buffer_range_entry(0, uniforms, 0, DRAW_BYTES)]);
    (uniforms, id)
}
