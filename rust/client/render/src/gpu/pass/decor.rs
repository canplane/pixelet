// pxl-render/gpu/pass/decor.rs

//! Decor as instanced draws of each variant's model; the decor's instances are
//! the instance buffer, and its index list says which instances show. A variant's
//! skin is its image's (gpu/pass/model.rs `skin_group`). A listed draw that
//! changed marks the tiles it covers where it was and where it is
//! (gpu/pass/mark.rs), its instances the upload it reads.

use crate::frame::draw::uniform_of;
use crate::frame::draw::{PIPELINE_DECOR, PIPELINE_GHOST_DECOR};
use crate::frame::shader::record::{PIPELINE_DECOR_DEPTH, PIPELINE_DECOR_MARK};
use crate::frame::shader::word::DECOR_GROUPS;

use crate::gpu::api::{Format, Gpu, Image, Stencil, TrackedRenderPass};
use crate::gpu::generated::Drawn;
use crate::gpu::pass::impostor::Extent;
use crate::gpu::pass::mark::{self, Marks, UploadCopies};
use crate::gpu::pass::model::{
    bind_mesh, draw_model, draw_models, put_instances, skin_group, skin_made, DrawUniforms,
    ModelDraw, Stage, DRAW_BYTES,
};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::upload::{Draws, Uploads};
use pxl_spec_core::generated;

const _: () = assert!(
    generated::GROUPS as usize <= DECOR_GROUPS,
    "a decor variant past the draw groups"
);

pub struct DecorPass {
    lit: Built,
    ghost: Built,
    depth: Built,
    uniforms: DrawUniforms,
    /// Each group's mesh, 0 for a model not yet registered, and its skin's image.
    groups: [(u32, Option<Image>); DECOR_GROUPS],
    /// The marks of the listed draws, opaque, which cast, and blended.
    marks: Marks,
    ghost_marks: Marks,
}

/// A mark's uniform, as the draw's (`put_instances`).
fn put_mark(buf: &mut [u8], (_, u): &mark::Drawn) {
    put_instances(buf, u, 0);
}

impl DecorPass {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines, align: u32) -> DecorPass {
        DecorPass {
            lit: pipelines.build_world(g, PIPELINE_DECOR),
            ghost: pipelines.build_world(g, PIPELINE_GHOST_DECOR),
            depth: pipelines.build_gated(
                g,
                PIPELINE_DECOR_DEPTH,
                Format::depth32float,
                Stencil::Gated,
            ),
            uniforms: DrawUniforms::new(DRAW_BYTES, align),
            groups: [(0, None); DECOR_GROUPS],
            marks: Marks::new(
                g,
                pipelines,
                PIPELINE_DECOR_MARK,
                align,
                (|d| d[0], put_mark),
            ),
            ghost_marks: Marks::new(
                g,
                pipelines,
                PIPELINE_DECOR_MARK,
                align,
                (|d| d[0], put_mark),
            ),
        }
    }

    /// The marks this frame of the listed draws (gpu/pass/mark.rs
    /// `Marks::prepare`), opaque and blended: whether any, and whether any
    /// of the opaque, which cast.
    pub fn prepare_marks(
        &mut self,
        g: &Gpu,
        draws: &Draws,
        uniforms: &[f32],
        touched: &[u32],
        uploads: &mut Uploads,
        kept: &UploadCopies,
    ) -> (bool, bool) {
        let opaque = draws.of(PIPELINE_DECOR);
        let casts = self
            .marks
            .prepare(g, opaque, uniforms, touched, uploads, kept);
        let ghost = draws.of(PIPELINE_GHOST_DECOR);
        let blended = self
            .ghost_marks
            .prepare(g, ghost, uniforms, touched, uploads, kept);
        (casts || blended, casts)
    }

    /// The marks `prepare_marks` made drawn into `pass`, `frame` their group
    /// 0 (tile_mark.wgsl): the opaque's alone where `casters`, as the light's
    /// map draws them, else the blended's too.
    pub fn draw_marks(
        &self,
        pass: &mut TrackedRenderPass,
        frame: u32,
        uploads: &Uploads,
        casters: bool,
    ) {
        let marks = if casters {
            &[&self.marks][..]
        } else {
            &[&self.marks, &self.ghost_marks][..]
        };
        for m in marks {
            m.draw(pass, frame, |pass, d, upload| {
                let Some((model, vertex_cnt, idx_cnt, skin)) = self.model_drawn(d[1], uploads)
                else {
                    return;
                };
                pass.set_bind_group(2, skin, &[]);
                pass.set_bind_group(3, upload, &[]);
                draw_model(pass, &self.lit, model, vertex_cnt, idx_cnt, d[2]);
            });
        }
    }

    /// The uploads the listed draws read, into `out` (`Marks::reads`).
    pub fn marks_read(&self, draws: &Draws, out: &mut Vec<u32>) {
        self.marks.reads(draws.of(PIPELINE_DECOR), out);
        self.ghost_marks.reads(draws.of(PIPELINE_GHOST_DECOR), out);
    }

    /// What this frame listed kept for the next's marks where it was.
    pub fn keep(&mut self, draws: &Draws, uniforms: &[f32]) {
        self.marks.keep(draws.of(PIPELINE_DECOR), uniforms);
        self.ghost_marks
            .keep(draws.of(PIPELINE_GHOST_DECOR), uniforms);
    }

    pub fn set_models(&mut self, groups: &[(u32, Option<Image>, Extent)]) {
        for (group, &(mesh, image, _)) in groups.iter().take(DECOR_GROUPS).enumerate() {
            self.groups[group] = (mesh, image);
        }
    }

    /// None while its mesh or its skin's image lands; its skin's group made
    /// here, as a frame prepares (gpu/pass/model.rs `skin_group`).
    pub(crate) fn model_of(
        &self,
        g: &Gpu,
        group: u32,
        uploads: &Uploads,
    ) -> Option<(u32, u32, u32, u32)> {
        self.model(group, uploads, |image| {
            skin_group(g, self.lit.layouts[2], image)
        })
    }

    /// As a frame's draws bind it: a skin its prepare made, or none.
    fn model_drawn(&self, group: u32, uploads: &Uploads) -> Option<(u32, u32, u32, u32)> {
        self.model(group, uploads, |image| skin_made(image.0))
    }

    fn model(
        &self,
        group: u32,
        uploads: &Uploads,
        skin: impl FnOnce(Image) -> Option<u32>,
    ) -> Option<(u32, u32, u32, u32)> {
        let (mesh, image) = *self.groups.get(group as usize)?;
        let (buf, vertex_cnt, idx_cnt) = uploads.get(mesh)?;
        Some((buf, vertex_cnt, idx_cnt, skin(image?)?))
    }

    /// 0 while it loads.
    pub fn triangles_of(&self, group: u32, uploads: &Uploads) -> u32 {
        self.groups
            .get(group as usize)
            .and_then(|&(mesh, _)| uploads.get(mesh))
            .map_or(0, |(_, _, idx_cnt)| idx_cnt / 3)
    }

    /// Every skin and upload group the draws will bind is made here, so drawing
    /// makes nothing that outlives its frame.
    /// `moved` whether the lists the draws are made from moved.
    pub fn prepare(
        &mut self,
        g: &Gpu,
        draws: &Draws,
        uniforms: &[f32],
        moved: bool,
        uploads: &mut Uploads,
    ) {
        let (opaque, ghost) = (draws.of(PIPELINE_DECOR), draws.of(PIPELINE_GHOST_DECOR));
        let cnt = (opaque.len() + ghost.len()) as u32;
        self.uniforms.write(
            g,
            self.lit.layouts[1],
            cnt,
            opaque.clone().chain(ghost.clone()),
            moved,
            |buf, d| {
                put_instances(buf, uniform_of(uniforms, d), 0);
            },
        );
        for d in opaque.chain(ghost) {
            self.model_of(g, d[1], uploads);
            uploads.group(g, d[0]);
        }
    }

    /// The listed decor, lit or its depth from the sun (`Stage::Depth`),
    /// `frame` the group 0 the stage reads.
    pub fn draw(
        &mut self,
        pass: &mut TrackedRenderPass,
        draws: &Draws,
        uploads: &mut Uploads,
        stage: Stage,
        frame: u32,
    ) {
        let pipeline = self.pipeline(stage);
        self.draw_pipeline(pass, draws, uploads, PIPELINE_DECOR, pipeline, frame);
    }

    fn pipeline(&self, stage: Stage) -> u32 {
        match stage {
            Stage::Lit => self.lit.pipeline,
            Stage::Depth => self.depth.pipeline,
        }
    }

    pub fn draw_ghosts(
        &mut self,
        pass: &mut TrackedRenderPass,
        draws: &Draws,
        uploads: &mut Uploads,
        frame: u32,
    ) {
        self.draw_pipeline(
            pass,
            draws,
            uploads,
            PIPELINE_GHOST_DECOR,
            self.ghost.pipeline,
            frame,
        );
    }

    /// The ghost's uniforms follow the opaque ones' (`prepare`).
    fn draw_pipeline(
        &mut self,
        pass: &mut TrackedRenderPass,
        draws: &Draws,
        uploads: &mut Uploads,
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
                let (model, vertex_cnt, idx_cnt, skin) = self.model_drawn(d[1], uploads)?;
                let upload = uploads.group_of(d[0])?;
                Some(ModelDraw {
                    groups: [skin, upload],
                    mesh: Some((model, vertex_cnt, idx_cnt)),
                    instances: d[2],
                })
            },
        );
    }

    /// The models the GPU generated, an indirect draw a group whose model is held
    /// (pxl-spec-core `generated::drawn`, a group's models its region `2·group`).
    pub fn draw_generated(
        &mut self,
        pass: &mut TrackedRenderPass,
        generated: Option<Drawn>,
        uploads: &Uploads,
        stage: Stage,
        frame: u32,
    ) {
        let Some(d) = generated else { return };
        pass.set_pipeline(self.pipeline(stage));
        pass.set_bind_group(0, frame, &[]);
        pass.set_bind_group(3, d.instances_group, &[]);
        for group in 0..generated::GROUPS {
            let Some((model, vertex_cnt, idx_cnt, skin)) = self.model_drawn(group, uploads) else {
                continue;
            };
            pass.set_bind_group(1, d.uniform_group, &[d.uniform_at(2 * group)]);
            pass.set_bind_group(2, skin, &[]);
            bind_mesh(pass, &self.lit, model, vertex_cnt, idx_cnt);
            pass.draw_indexed_indirect(d.args, d.args_at(2 * group));
        }
    }
}
