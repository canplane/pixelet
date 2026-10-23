// pxl-render/gpu/pass/skinned.rs

//! Actors: each model's mesh block skinned by each actor's joint palette. A
//! case of the variant picker draws one actor in a pass of its own.

use crate::frame::draw::uniform_of;
use crate::frame::draw::{DRAW_WORDS, PIPELINE_SKINNED};
use crate::gpu::renderer::Image;

use crate::gpu::api::{bytes, Gpu};

use crate::gpu::pass::model::{draw_models, DrawUniforms, ModelDraw, DRAW_BYTES};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::renderer::{skin_group, Blocks};

pub struct SkinnedPass {
    lit: Built,
    uniforms: DrawUniforms,
    /// Each model's skin image, as the frame hands them.
    skins: Vec<(u32, Image)>,
}

impl SkinnedPass {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines, align: u32) -> SkinnedPass {
        SkinnedPass {
            lit: pipelines.build(g, PIPELINE_SKINNED, g.canvas()),
            uniforms: DrawUniforms::new(DRAW_BYTES, align),
            skins: Vec::new(),
        }
    }

    pub fn set_skins(&mut self, skins: &[(u32, Image)]) {
        self.skins.clear();
        self.skins.extend_from_slice(skins);
    }

    /// None while its image lands, and the actor is drawn on a later frame.
    fn skin_of(&self, g: &Gpu, model: u32) -> Option<u32> {
        let &(_, image) = self.skins.iter().find(|&&(m, _)| m == model)?;
        skin_group(g, self.lit.layouts[2], image)
    }

    /// Every palette group the draws will bind is made here, so drawing makes
    /// nothing that outlives its frame.
    /// `moved` whether the lists the draws are made from moved.
    pub fn prepare<'a>(&mut self, g: &Gpu, draws: impl ExactSizeIterator<Item = &'a [u32; DRAW_WORDS]> + Clone, uniforms: &[f32], moved: bool, blocks: &mut Blocks) {
        self.uniforms.write(g, self.lit.layouts[1], draws.len() as u32, draws.clone(), moved, |buf, d| {
            let u = uniform_of(uniforms, d);
            // Translation, turn about Z, opacity, scale, padding (skinned.wgsl `Draw`).
            buf[..24].copy_from_slice(bytes(&u[..6]));
            buf[24..DRAW_BYTES as usize].fill(0);
        });
        for d in draws {
            blocks.group(g, d[1], self.lit.layouts[3]);
        }
    }

    pub fn draw<'a>(&self, g: &Gpu, pass: u32, draws: impl Iterator<Item = &'a [u32; DRAW_WORDS]>, blocks: &mut Blocks, frame: (u32, &mut u32)) {
        let Some(slots) = self.uniforms.slots() else { return };
        let palette_layout = self.lit.layouts[3];
        draw_models(g, pass, &self.lit, self.lit.pipeline, frame, slots, 0, draws, |d| {
            let (mesh, vertex_cnt, index_cnt) = blocks.get(d[0])?;
            let skin = self.skin_of(g, d[2])?;
            Some(ModelDraw {
                groups: [skin, blocks.group(g, d[1], palette_layout)?],
                mesh: Some((mesh, vertex_cnt, index_cnt)),
                instances: 1,
            })
        });
    }

    pub fn release(mut self) {
        self.uniforms.release();
    }
}
