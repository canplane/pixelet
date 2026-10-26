// pxl-render/gpu/pass/skinned.rs

//! Actors: each model's mesh upload skinned by each actor's joint palette. A
//! case of the variant picker draws one actor in a pass of its own.

use crate::frame::draw::uniform_of;
use crate::frame::draw::{DRAW_WORDS, PIPELINE_SKINNED};
use crate::frame::shader::record::PIPELINE_SKINNED_DEPTH;

use crate::gpu::api::{bytes, Gpu, Image};
use crate::gpu::pass::model::{
    draw_models, skin_group, skin_made, DrawUniforms, ModelDraw, Stage, DRAW_BYTES,
};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::upload::Uploads;

pub struct SkinnedPass {
    lit: Built,
    depth: Built,
    uniforms: DrawUniforms,
    /// Each model's skin image, as the frame hands them.
    skins: Vec<(u32, Image)>,
}

impl SkinnedPass {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines, align: u32) -> SkinnedPass {
        SkinnedPass {
            lit: pipelines.build(g, PIPELINE_SKINNED, g.canvas()),
            depth: pipelines.build(g, PIPELINE_SKINNED_DEPTH, g.canvas()),
            uniforms: DrawUniforms::new(DRAW_BYTES, align),
            skins: Vec::new(),
        }
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
    #[allow(clippy::too_many_arguments)]
    pub fn draw<'a>(
        &self,
        g: &Gpu,
        pass: u32,
        draws: impl Iterator<Item = &'a [u32; DRAW_WORDS]>,
        uploads: &mut Uploads,
        stage: Stage,
        frame: (u32, &mut u32),
    ) {
        let Some(slots) = self.uniforms.slots() else {
            return;
        };
        let pipeline = match stage {
            Stage::Depth => self.depth.pipeline,
            _ => self.lit.pipeline,
        };
        draw_models(g, pass, &self.lit, pipeline, frame, slots, 0, draws, |d| {
            let (mesh, vertex_cnt, index_cnt) = uploads.get(d[0])?;
            let skin = skin_made(self.image_of(d[2])?.0)?;
            Some(ModelDraw {
                groups: [skin, uploads.group(g, d[1])?],
                mesh: Some((mesh, vertex_cnt, index_cnt)),
                instances: 1,
            })
        });
    }

    pub fn release(mut self) {
        self.uniforms.release();
    }
}
