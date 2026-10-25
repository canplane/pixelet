// pxl-render/gpu/pass/decor.rs

//! Decor as instanced draws of each variant's model; the decor's instances are
//! the instance buffer, and its index list says which instances show. A variant's
//! skin is its image's (gpu/pass/model.rs `skin_group`).

use crate::frame::draw::uniform_of;
use crate::frame::draw::{PIPELINE_DECOR, PIPELINE_GHOST_DECOR};
use crate::frame::shader::record::{PIPELINE_DECOR_DEPTH, PIPELINE_DECOR_PREPASS};

use crate::gpu::api::{Gpu, Image};
use crate::gpu::grown::Drawn;
use crate::gpu::pass::impostor::Extent;
use crate::gpu::pass::model::{bind_mesh, draw_models, put_instances, skin_group, skin_made, DrawUniforms, ModelDraw, Stage, DRAW_BYTES};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::upload::{Draws, Uploads};

pub const GROUPS: usize = 64;

pub struct DecorPass {
    lit: Built,
    ghost: Built,
    depth: Built,
    prepass: Built,
    uniforms: DrawUniforms,
    /// Each group's mesh, 0 for a model not yet registered, and its skin's image.
    groups: [(u32, Option<Image>); GROUPS],
}

impl DecorPass {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines, align: u32) -> DecorPass {
        DecorPass {
            lit: pipelines.build(g, PIPELINE_DECOR, g.canvas()),
            ghost: pipelines.build(g, PIPELINE_GHOST_DECOR, g.canvas()),
            depth: pipelines.build(g, PIPELINE_DECOR_DEPTH, g.canvas()),
            prepass: pipelines.build(g, PIPELINE_DECOR_PREPASS, g.canvas()),
            uniforms: DrawUniforms::new(DRAW_BYTES, align),
            groups: [(0, None); GROUPS],
        }
    }

    pub fn set_models(&mut self, groups: &[(u32, Option<Image>, Extent)]) {
        for (group, &(mesh, image, _)) in groups.iter().take(GROUPS).enumerate() {
            self.groups[group] = (mesh, image);
        }
    }

    /// None while its mesh or its skin's image lands; its skin's group made
    /// here, as a frame prepares (gpu/pass/model.rs `skin_group`).
    pub(crate) fn model_of(&self, g: &Gpu, group: u32, uploads: &Uploads) -> Option<(u32, u32, u32, u32)> {
        self.model(group, uploads, |image| skin_group(g, self.lit.layouts[2], image))
    }

    /// As a frame's draws bind it: a skin its prepare made, or none.
    fn model_drawn(&self, group: u32, uploads: &Uploads) -> Option<(u32, u32, u32, u32)> {
        self.model(group, uploads, |image| skin_made(image.0))
    }

    fn model(&self, group: u32, uploads: &Uploads, skin: impl FnOnce(Image) -> Option<u32>) -> Option<(u32, u32, u32, u32)> {
        let (mesh, image) = *self.groups.get(group as usize)?;
        let (buffer, vertex_cnt, index_cnt) = uploads.get(mesh)?;
        Some((buffer, vertex_cnt, index_cnt, skin(image?)?))
    }

    /// 0 while it loads.
    pub fn triangles_of(&self, group: u32, uploads: &Uploads) -> u32 {
        self.groups.get(group as usize).and_then(|&(mesh, _)| uploads.get(mesh)).map_or(0, |(_, _, index_cnt)| index_cnt / 3)
    }

    /// Every skin and upload group the draws will bind is made here, so drawing
    /// makes nothing that outlives its frame.
    /// `moved` whether the lists the draws are made from moved.
    pub fn prepare(&mut self, g: &Gpu, draws: &Draws, uniforms: &[f32], moved: bool, uploads: &mut Uploads) {
        let (opaque, ghost) = (draws.of(PIPELINE_DECOR), draws.of(PIPELINE_GHOST_DECOR));
        let count = (opaque.len() + ghost.len()) as u32;
        self.uniforms.write(g, self.lit.layouts[1], count, opaque.clone().chain(ghost.clone()), moved, |buf, d| {
            put_instances(buf, uniform_of(uniforms, d), 0);
        });
        for d in opaque.chain(ghost) {
            self.model_of(g, d[1], uploads);
            uploads.group(g, d[0]);
        }
    }

    /// The listed decor, lit or its depth from the sun (`Stage::Depth`),
    /// `frame` the group 0 the stage reads.
    pub fn draw(&mut self, g: &Gpu, pass: u32, draws: &Draws, uploads: &mut Uploads, stage: Stage, frame: (u32, &mut u32)) {
        let pipeline = self.pipeline(stage);
        self.draw_pipeline(g, pass, draws, uploads, PIPELINE_DECOR, pipeline, frame);
    }

    fn pipeline(&self, stage: Stage) -> u32 {
        match stage {
            Stage::Lit => self.lit.pipeline,
            Stage::Depth => self.depth.pipeline,
            Stage::Prepass => self.prepass.pipeline,
        }
    }

    pub fn draw_ghosts(&mut self, g: &Gpu, pass: u32, draws: &Draws, uploads: &mut Uploads, frame: (u32, &mut u32)) {
        self.draw_pipeline(g, pass, draws, uploads, PIPELINE_GHOST_DECOR, self.ghost.pipeline, frame);
    }

    /// The ghost's uniforms follow the opaque ones' (`prepare`).
    #[allow(clippy::too_many_arguments)]
    fn draw_pipeline(&mut self, g: &Gpu, pass: u32, draws: &Draws, uploads: &mut Uploads, which: u32, pipeline: u32, frame: (u32, &mut u32)) {
        let Some(slots) = self.uniforms.slots() else { return };
        let first = if which == PIPELINE_GHOST_DECOR { draws.of(PIPELINE_DECOR).len() as u32 } else { 0 };
        let lit = self.lit;
        draw_models(g, pass, &lit, pipeline, frame, slots, first, draws.of(which), |d| {
            let (model, vertex_cnt, index_cnt, skin) = self.model_drawn(d[1], uploads)?;
            let upload = uploads.group(g, d[0])?;
            Some(ModelDraw {
                groups: [skin, upload],
                mesh: Some((model, vertex_cnt, index_cnt)),
                instances: d[2],
            })
        });
    }

    /// The models the GPU grew, an indirect draw a group whose model is held
    /// (pxl-spec-core `scatter::drawn`, a group's models its region `2·group`).
    pub fn draw_grown(&mut self, g: &Gpu, pass: u32, grown: Option<Drawn>, uploads: &Uploads, stage: Stage, (frame, bound): (u32, &mut u32)) {
        let Some(d) = grown else { return };
        g.set_pipeline(pass, self.pipeline(stage));
        if *bound != frame {
            g.set_bind_group(pass, 0, frame, &[]);
            *bound = frame;
        }
        g.set_bind_group(pass, 3, d.instances_group, &[]);
        let mut mesh_bound = 0;
        for group in 0..pxl_spec_core::scatter::GROUPS {
            let Some((model, vertex_cnt, index_cnt, skin)) = self.model_drawn(group, uploads) else { continue };
            g.set_bind_group(pass, 1, d.uniform_group, &[d.uniform_at(2 * group)]);
            g.set_bind_group(pass, 2, skin, &[]);
            bind_mesh(g, pass, &self.lit, &mut mesh_bound, model, vertex_cnt, index_cnt);
            g.draw_indexed_indirect(pass, d.args, d.args_at(2 * group));
        }
    }
}
