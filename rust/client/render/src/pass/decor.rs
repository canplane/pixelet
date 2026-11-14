// pxl-render/pass/decor.rs

//! The decor's model tier. Each decor variant is a draw group with one model
//! and one skin, and the pass draws a group's model instanced. The decor's
//! instances are the instance buffer, and its index list says which of them
//! show. A group's skin is the bind group of its image (pass/model.rs
//! `skin_group`). A listed draw that changed marks the tiles it covers where
//! it was and where it is (pass/mark.rs). The upload such a draw reads
//! holds its instances.

use crate::frame::shader::record::{PIPELINE_DECOR, PIPELINE_GHOST_DECOR};
use crate::frame::shader::record::{PIPELINE_DECOR_DEPTH, PIPELINE_DECOR_MARK};
use crate::frame::shader::word::DECOR_GROUP_CNT;
use crate::gpu::draw::uniform_of;
use crate::gpu::draw::DrawStat;

use crate::frame::shader::sequence;
use crate::gpu::api::{Format, Gpu, Image, Stencil, TrackedRenderPass};
use crate::gpu::draw::DrawTable;
use crate::gpu::generator::IndirectDraw;
use crate::gpu::pipeline::{Pipeline, PipelineCache};
use crate::gpu::upload::UploadTable;
use crate::pass::mark::{put_whole, MarkPass};
use crate::pass::model::{
    bind_mesh, draw_model, draw_models, put_uniform, skin_group, skin_made, Extent, ModelDraw,
    Stage, UniformBuffer, DRAW_BYTES,
};
use pxl_content::generation;

const _: () = assert!(
    generation::GROUP_CNT as usize <= DECOR_GROUP_CNT,
    "a decor variant past the draw groups"
);

pub struct DecorPass {
    lit: Pipeline,
    ghost: Pipeline,
    depth: Pipeline,
    uniforms: UniformBuffer,
    /// Each group's mesh, 0 while its model is not registered, and the image of
    /// its skin.
    groups: [(u32, Option<Image>); DECOR_GROUP_CNT],
    /// The marks of the listed opaque draws, which cast shadows.
    pub mark: MarkPass,
    /// The marks of the listed ghost draws, which are blended.
    pub ghost_mark: MarkPass,
}

impl DecorPass {
    pub fn new(g: &Gpu, pipelines: &mut PipelineCache, align: u32) -> DecorPass {
        DecorPass {
            lit: pipelines.build_world(g, PIPELINE_DECOR),
            ghost: pipelines.build_world(g, PIPELINE_GHOST_DECOR),
            depth: pipelines.build_masked(
                g,
                PIPELINE_DECOR_DEPTH,
                Format::Depth32float,
                Stencil::Masked,
            ),
            uniforms: UniformBuffer::new(DRAW_BYTES, align),
            groups: [(0, None); DECOR_GROUP_CNT],
            mark: MarkPass::new(
                g,
                pipelines,
                (PIPELINE_DECOR_MARK, PIPELINE_DECOR),
                align,
                (|d| d[0], put_whole),
            ),
            ghost_mark: MarkPass::new(
                g,
                pipelines,
                (PIPELINE_DECOR_MARK, PIPELINE_GHOST_DECOR),
                align,
                (|d| d[0], put_whole),
            ),
        }
    }

    /// Draws this frame's marks into `pass`. `frames` holds their group 0 as
    /// they are and as they were (tile_mark.wgsl, `MarkPass::draw`). Where
    /// `casters`, as for the light's map, only the opaque draws' marks are
    /// drawn; otherwise the ghosts' are drawn too.
    pub fn draw_marks(
        &self,
        pass: &mut TrackedRenderPass,
        frames: [u32; 2],
        uploads: &UploadTable,
        casters: bool,
    ) {
        let marks = if casters {
            &[&self.mark][..]
        } else {
            &[&self.mark, &self.ghost_mark][..]
        };
        for m in marks {
            m.draw(pass, frames, |pass, d, upload| {
                let Some((model, vertex_cnt, idx_cnt, skin)) = self.model_binding(d[1], uploads)
                else {
                    return;
                };
                pass.set_bind_group(2, skin, &[]);
                pass.set_bind_group(3, upload, &[]);
                draw_model(pass, &self.lit, model, vertex_cnt, idx_cnt, d[2]);
            });
        }
    }

    /// Pushes onto `out` the uploads the listed draws read (`MarkPass::reads`).
    pub fn marks_read(&self, draws: &DrawTable, out: &mut Vec<u32>) {
        self.mark.reads(draws.of(PIPELINE_DECOR), out);
        self.ghost_mark.reads(draws.of(PIPELINE_GHOST_DECOR), out);
    }

    pub fn set_models(&mut self, groups: &[(u32, Option<Image>, Extent)]) {
        for (group, &(mesh, image, _)) in groups.iter().take(DECOR_GROUP_CNT).enumerate() {
            self.groups[group] = (mesh, image);
        }
    }

    /// The mesh buffer of `group`'s model, its vertex and index counts and its
    /// skin group, or none while the mesh or the skin's image is landing. It
    /// makes the skin group where there is none, so it is called only while
    /// a frame prepares (pass/model.rs `skin_group`).
    pub(crate) fn model_of(
        &self,
        g: &Gpu,
        group: u32,
        uploads: &UploadTable,
    ) -> Option<(u32, u32, u32, u32)> {
        self.model(group, uploads, |image| {
            skin_group(g, self.lit.layouts[2], image)
        })
    }

    /// What `model_of` gives, as a frame's draws bind it: it takes the skin
    /// group a prepare made and makes none.
    fn model_binding(&self, group: u32, uploads: &UploadTable) -> Option<(u32, u32, u32, u32)> {
        self.model(group, uploads, |image| skin_made(image.0))
    }

    fn model(
        &self,
        group: u32,
        uploads: &UploadTable,
        skin: impl FnOnce(Image) -> Option<u32>,
    ) -> Option<(u32, u32, u32, u32)> {
        let (mesh, image) = *self.groups.get(group as usize)?;
        let (buf, vertex_cnt, idx_cnt) = uploads.get(mesh)?;
        Some((buf, vertex_cnt, idx_cnt, skin(image?)?))
    }

    /// The triangles of `group`'s model, 0 while its mesh is landing.
    fn triangles_of(&self, group: u32, uploads: &UploadTable) -> u32 {
        self.groups
            .get(group as usize)
            .and_then(|&(mesh, _)| uploads.get(mesh))
            .map_or(0, |(_, _, idx_cnt)| idx_cnt / 3)
    }

    /// Adds the pass's draws to `stat`. Each instance counts the triangles of
    /// its group's model.
    pub fn count(&self, draws: &DrawTable, uploads: &UploadTable, stat: &mut DrawStat) {
        for pipeline in [PIPELINE_DECOR, PIPELINE_GHOST_DECOR] {
            draws.count(pipeline, stat, |d| self.triangles_of(d[1], uploads) * d[2]);
        }
    }

    /// Writes the draws' uniforms and makes every skin and upload group the
    /// draws will bind, so that drawing makes none: a group made inside a
    /// frame that is dropped would be gone. `changed` says whether the lists
    /// the draws are made from moved.
    pub fn prepare(
        &mut self,
        g: &Gpu,
        draws: &DrawTable,
        uniforms: &[f32],
        changed: bool,
        uploads: &mut UploadTable,
    ) {
        let (opaque, ghost) = (draws.of(PIPELINE_DECOR), draws.of(PIPELINE_GHOST_DECOR));
        let cnt = (opaque.len() + ghost.len()) as u32;
        self.uniforms.write(
            g,
            self.lit.layouts[1],
            cnt,
            opaque.clone().chain(ghost.clone()),
            changed,
            |buf, d| {
                put_uniform(buf, uniform_of(uniforms, d));
            },
        );
        for d in opaque.chain(ghost) {
            self.model_of(g, d[1], uploads);
            uploads.group(g, d[0]);
        }
    }

    /// Draws the listed opaque decor at `stage`: lit, or its depth seen from
    /// the light. `frame` is the group 0 the stage reads.
    pub fn draw(
        &mut self,
        pass: &mut TrackedRenderPass,
        draws: &DrawTable,
        uploads: &mut UploadTable,
        stage: Stage,
        frame: u32,
    ) {
        let pipeline = self.pipeline(stage);
        self.draw_pipeline(pass, draws, uploads, PIPELINE_DECOR, pipeline, frame);
    }

    fn pipeline(&self, stage: Stage) -> u32 {
        match stage {
            Stage::Lit => self.lit.id,
            Stage::Depth => self.depth.id,
        }
    }

    pub fn draw_ghosts(
        &mut self,
        pass: &mut TrackedRenderPass,
        draws: &DrawTable,
        uploads: &mut UploadTable,
        frame: u32,
    ) {
        self.draw_pipeline(
            pass,
            draws,
            uploads,
            PIPELINE_GHOST_DECOR,
            self.ghost.id,
            frame,
        );
    }

    /// The ghosts' uniform slots follow the opaque draws' slots (`prepare`),
    /// so a ghost's slot is offset by the count of opaque draws.
    fn draw_pipeline(
        &mut self,
        pass: &mut TrackedRenderPass,
        draws: &DrawTable,
        uploads: &mut UploadTable,
        which: u32,
        pipeline: u32,
        frame: u32,
    ) {
        let Some(slots) = self.uniforms.slots() else {
            return;
        };
        let first = if which == PIPELINE_GHOST_DECOR {
            draws.of(PIPELINE_DECOR).len() as u32
        } else {
            0
        };
        let lit = self.lit;
        draw_models(
            pass,
            &lit,
            pipeline,
            frame,
            slots.skip(first),
            draws.of(which),
            |d| {
                let (model, vertex_cnt, idx_cnt, skin) = self.model_binding(d[1], uploads)?;
                let upload = uploads.group_of(d[0])?;
                Some(ModelDraw {
                    groups: [skin, upload],
                    mesh: Some((model, vertex_cnt, idx_cnt)),
                    instance_cnt: d[2],
                })
            },
        );
    }

    /// Draws the models the GPU generated, one indirect draw for each group
    /// whose model is on the device. A group's models are in region `2·group`
    /// of what the GPU generated (pxl-content `generation::drawn`).
    pub fn draw_generated(
        &mut self,
        pass: &mut TrackedRenderPass,
        generated: Option<IndirectDraw>,
        uploads: &UploadTable,
        stage: Stage,
        frame: u32,
    ) {
        let Some(d) = generated else { return };
        let at = match (stage, d.mask) {
            (Stage::Depth, None) => const { sequence::DECOR_MAP.at() },
            (Stage::Depth, Some(_)) => const { sequence::DECOR_MAP_MASKED.at() },
            (Stage::Lit, None) => const { sequence::DECOR_LIT.at() },
            (Stage::Lit, Some(_)) => const { sequence::DECOR_LIT_MASKED.at() },
        };
        pass.set_pipeline(self.pipeline(stage));
        pass.set_bind_group(0, frame, &[]);
        pass.set_bind_group(3, d.instances_group, &[]);
        for group in 0..generation::GROUP_CNT {
            let Some((model, vertex_cnt, idx_cnt, skin)) = self.model_binding(group, uploads)
            else {
                continue;
            };
            pass.set_bind_group(1, d.uniform_group, &[d.uniform_at(2 * group)]);
            pass.set_bind_group(2, skin, &[]);
            bind_mesh(pass, &self.lit, model, vertex_cnt, idx_cnt);
            pass.draw_indexed_indirect(d.args, at + d.args_at(2 * group));
        }
    }
}
