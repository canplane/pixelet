// pxl-render/gpu/pass/skinned.rs

//! Actors: each model's mesh upload skinned by each actor's joint palette. A
//! case of the variant picker draws one actor in a pass of its own. An actor
//! whose draw changed marks the tiles it covers where it was and where it is
//! (gpu/pass/mark.rs), its palette the upload it reads.

use crate::frame::draw::uniform_of;
use crate::frame::draw::{DRAW_WORDS, PIPELINE_SKINNED};
use crate::frame::shader::record::{PIPELINE_SKINNED_DEPTH, PIPELINE_SKINNED_MARK};

use crate::gpu::api::{bytes, Format, Gpu, Image, Stencil, TrackedRenderPass};
use crate::gpu::pass::mark::{Drawn, Marks, UploadCopies};
use crate::gpu::pass::model::{
    draw_model, draw_models, skin_group, skin_made, DrawUniforms, ModelDraw, Stage, DRAW_BYTES,
};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::upload::Uploads;

pub struct SkinnedPass {
    lit: Built,
    depth: Built,
    uniforms: DrawUniforms,
    /// Each model's skin image, as the frame hands them.
    skins: Vec<(u32, Image)>,
    marks: Marks,
}

/// A mark's uniform: its place, the uniform's first words (skinned.wgsl
/// `Draw`).
fn put_mark(buf: &mut [u8], (_, u): &Drawn) {
    buf[..24].copy_from_slice(bytes(&u[..6]));
    buf[24..DRAW_BYTES as usize].fill(0);
}

impl SkinnedPass {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines, align: u32) -> SkinnedPass {
        SkinnedPass {
            lit: pipelines.build_world(g, PIPELINE_SKINNED),
            depth: pipelines.build_gated(
                g,
                PIPELINE_SKINNED_DEPTH,
                Format::depth32float,
                Stencil::Gated,
            ),
            uniforms: DrawUniforms::new(DRAW_BYTES, align),
            skins: Vec::new(),
            marks: Marks::new(
                g,
                pipelines,
                PIPELINE_SKINNED_MARK,
                align,
                (|d| d[1], put_mark),
            ),
        }
    }

    /// The marks this frame of `draws` (gpu/pass/mark.rs `Marks::prepare`),
    /// each reading its palette. Whether any.
    pub fn prepare_marks<'a>(
        &mut self,
        g: &Gpu,
        draws: impl Iterator<Item = &'a [u32; DRAW_WORDS]>,
        uniforms: &[f32],
        touched: &[u32],
        uploads: &mut Uploads,
        kept: &UploadCopies,
    ) -> bool {
        self.marks
            .prepare(g, draws, uniforms, touched, uploads, kept)
    }

    /// Group 0's layout of the marks (tile_mark.wgsl).
    pub fn mark_layout(&self) -> u32 {
        self.marks.layout()
    }

    /// The marks `prepare_marks` made drawn into `pass`, `frame` their group
    /// 0: the eye's or the light's frame and its tiles' (tile_mark.wgsl).
    pub fn draw_marks(&self, pass: &mut TrackedRenderPass, frame: u32, uploads: &Uploads) {
        self.marks.draw(pass, frame, |pass, d, palette| {
            let Some((mesh, vertex_cnt, idx_cnt)) = uploads.get(d[0]) else {
                return;
            };
            let Some(skin) = self.image_of(d[2]).and_then(|i| skin_made(i.0)) else {
                return;
            };
            pass.set_bind_group(2, skin, &[]);
            pass.set_bind_group(3, palette, &[]);
            draw_model(pass, &self.lit, mesh, vertex_cnt, idx_cnt, 1);
        });
    }

    /// The uploads `draws` read, into `out` (`Marks::reads`).
    pub fn marks_read<'a>(
        &self,
        draws: impl Iterator<Item = &'a [u32; DRAW_WORDS]>,
        out: &mut Vec<u32>,
    ) {
        self.marks.reads(draws, out);
    }

    /// What this frame drew of `draws` kept for the next's marks where it
    /// was.
    pub fn keep<'a>(
        &mut self,
        draws: impl Iterator<Item = &'a [u32; DRAW_WORDS]>,
        uniforms: &[f32],
    ) {
        self.marks.keep(draws, uniforms);
    }

    pub fn set_skins(&mut self, skins: &[(u32, Image)]) {
        self.skins.clear();
        self.skins.extend_from_slice(skins);
    }

    fn image_of(&self, model: u32) -> Option<Image> {
        self.skins
            .iter()
            .find(|&&(m, _)| m == model)
            .map(|&(_, image)| image)
    }

    /// Every palette and skin group the draws will bind is made here, so
    /// drawing makes nothing that outlives its frame; a skin whose image
    /// lands later is drawn on a later frame.
    /// `moved` whether the lists the draws are made from moved.
    pub fn prepare<'a>(
        &mut self,
        g: &Gpu,
        draws: impl ExactSizeIterator<Item = &'a [u32; DRAW_WORDS]> + Clone,
        uniforms: &[f32],
        moved: bool,
        uploads: &mut Uploads,
    ) {
        self.uniforms.write(
            g,
            self.lit.layouts[1],
            draws.len() as u32,
            draws.clone(),
            moved,
            |buf, d| {
                let u = uniform_of(uniforms, d);
                // Translation, turn about Z, opacity, scale, padding (skinned.wgsl `Draw`).
                buf[..24].copy_from_slice(bytes(&u[..6]));
                buf[24..DRAW_BYTES as usize].fill(0);
            },
        );
        for d in draws {
            uploads.group(g, d[1]);
            if let Some(image) = self.image_of(d[2]) {
                skin_group(g, self.lit.layouts[2], image);
            }
        }
    }

    /// Lit, or their depth from the sun (`Stage::Depth`), `frame` the group
    /// 0 the stage reads.
    pub fn draw<'a>(
        &self,
        pass: &mut TrackedRenderPass,
        draws: impl Iterator<Item = &'a [u32; DRAW_WORDS]>,
        uploads: &mut Uploads,
        stage: Stage,
        frame: u32,
    ) {
        let Some(slots) = self.uniforms.slots() else {
            return;
        };
        let pipeline = match stage {
            Stage::Depth => self.depth.pipeline,
            _ => self.lit.pipeline,
        };
        draw_models(pass, &self.lit, pipeline, frame, slots, draws, |d| {
            let (mesh, vertex_cnt, idx_cnt) = uploads.get(d[0])?;
            let skin = skin_made(self.image_of(d[2])?.0)?;
            Some(ModelDraw {
                groups: [skin, uploads.group_of(d[1])?],
                mesh: Some((mesh, vertex_cnt, idx_cnt)),
                instances: 1,
            })
        });
    }

    pub fn release(mut self) {
        self.uniforms.release();
        self.marks.release();
    }
}
