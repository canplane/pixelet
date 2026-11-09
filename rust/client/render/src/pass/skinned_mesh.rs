// pxl-render/pass/skinned_mesh.rs

//! The characters. A character's model is a mesh upload, skinned by the
//! character's joint palette, which is an upload of its own. A character's
//! vertices read its draw off the list's uniforms at its slot. A canvas of the
//! character picker, a character surface (pass/character.rs), draws one
//! character with a skinned pass of its own. A character whose draw changed
//! marks the tiles it covers where it was and where it is (pass/mark.rs).
//! The upload its mark reads is its palette.

use crate::frame::shader::record::PIPELINE_SKINNED;
use crate::frame::shader::record::{PIPELINE_SKINNED_DEPTH, PIPELINE_SKINNED_MARK};
use crate::gpu::draw::{DrawStat, DrawTable, DRAW_WORDS};

use crate::gpu::api::{buf_entry, release, Format, Gpu, Image, Layout, Stencil, TrackedRenderPass};
use crate::gpu::pipeline::{Pipeline, PipelineCache};
use crate::gpu::upload::UploadTable;
use crate::pass::mark::{put_whole, MarkPass};
use crate::pass::model::{bind_mesh, draw_model, skin_group, skin_made, Stage};

pub struct SkinnedPass {
    lit: Pipeline,
    depth: Pipeline,
    /// The bind groups through which the lit and the depth pipelines read the
    /// list's uniforms, and the buffer they are made over (`set_draws`).
    draws: [u32; 2],
    draws_buf: u32,
    /// The skin image of each model, as the frame hands them (`set_skins`).
    skins: Vec<(u32, Image)>,
    pub mark: MarkPass,
}

impl SkinnedPass {
    pub fn new(g: &Gpu, pipelines: &mut PipelineCache, align: u32) -> SkinnedPass {
        SkinnedPass {
            lit: pipelines.build_world(g, PIPELINE_SKINNED),
            depth: pipelines.build_masked(
                g,
                PIPELINE_SKINNED_DEPTH,
                Format::depth32float,
                Stencil::Masked,
            ),
            draws: [0; 2],
            draws_buf: 0,
            skins: Vec::new(),
            mark: MarkPass::new(
                g,
                pipelines,
                (PIPELINE_SKINNED_MARK, PIPELINE_SKINNED),
                align,
                (|d| d[1], put_whole, |_| true),
            ),
        }
    }

    /// The layout of the marks' group 0 (tile_mark.wgsl).
    pub fn mark_layout(&self) -> Layout {
        self.mark.layout()
    }

    /// Draws this frame's marks into `pass`. `frames` holds their group 0 as
    /// they are and as they were, over the eye's or the light's frame words
    /// and its tiles (tile_mark.wgsl, `MarkPass::draw`).
    pub fn draw_marks(
        &self,
        pass: &mut TrackedRenderPass,
        frames: [u32; 2],
        uploads: &UploadTable,
    ) {
        self.mark.draw(pass, frames, |pass, d, palette| {
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

    /// Pushes onto `out` the uploads `draws` read (`MarkPass::reads`).
    pub fn marks_read<'a>(
        &self,
        draws: impl Iterator<Item = &'a [u32; DRAW_WORDS]>,
        out: &mut Vec<u32>,
    ) {
        self.mark.reads(draws, out);
    }

    /// Adds the pass's draws to `stat`. Each counts its model's triangles, or
    /// 0 while its mesh is landing.
    pub fn count(&self, draws: &DrawTable, uploads: &UploadTable, stat: &mut DrawStat) {
        draws.count(PIPELINE_SKINNED, stat, |d| {
            uploads.get(d[0]).map_or(0, |(_, _, idx_cnt)| idx_cnt / 3)
        });
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

    /// Makes the groups 1 through which the lit and the depth pipelines read
    /// the list's uniforms, over the buffer `buf`. It makes them again only
    /// when the buffer is another. The caller calls it before the frame's
    /// commands, since a group made inside a frame that is dropped would be
    /// gone.
    pub fn set_draws(&mut self, g: &Gpu, buf: u32) {
        if buf == self.draws_buf {
            return;
        }
        for group in self.draws {
            release(group);
        }
        let layouts = [self.lit.layouts[1], self.depth.layouts[1]];
        self.draws = layouts.map(|l| g.bind_group(l, &[buf_entry(0, buf)]));
        self.draws_buf = buf;
    }

    /// Makes every palette and skin group the draws will bind, so that drawing
    /// makes none: a group made inside a frame that is dropped would be gone.
    /// A character whose skin image has not landed is drawn on a later frame.
    pub fn prepare<'a>(
        &mut self,
        g: &Gpu,
        draws: impl Iterator<Item = &'a [u32; DRAW_WORDS]>,
        uploads: &mut UploadTable,
    ) {
        for d in draws {
            uploads.group(g, d[1]);
            if let Some(image) = self.image_of(d[2]) {
                skin_group(g, self.lit.layouts[2], image);
            }
        }
    }

    /// Draws the characters at `stage`: lit, or their depth seen from the light.
    /// `frame` is the group 0 the stage reads. The list's uniforms are bound
    /// once (`set_draws`). Each character is drawn as the instance its slot
    /// names, and its vertices read their draw at that slot
    /// (skinned_draw.slot.wgsl). Each binds its own mesh, skin and palette.
    pub fn draw<'a>(
        &self,
        pass: &mut TrackedRenderPass,
        draws: impl Iterator<Item = &'a [u32; DRAW_WORDS]>,
        uploads: &mut UploadTable,
        stage: Stage,
        frame: u32,
    ) {
        let (pipeline, group) = match stage {
            Stage::Depth => (self.depth.id, self.draws[1]),
            _ => (self.lit.id, self.draws[0]),
        };
        if group == 0 {
            return;
        }
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, frame, &[]);
        pass.set_bind_group(1, group, &[]);
        for d in draws {
            let Some((mesh, vertex_cnt, idx_cnt)) = uploads.get(d[0]) else {
                continue;
            };
            let skin = self.image_of(d[2]).and_then(|i| skin_made(i.0));
            let (Some(skin), Some(palette)) = (skin, uploads.group_of(d[1])) else {
                continue;
            };
            pass.set_bind_group(2, skin, &[]);
            pass.set_bind_group(3, palette, &[]);
            bind_mesh(pass, &self.lit, mesh, vertex_cnt, idx_cnt);
            pass.draw_indexed(idx_cnt, 1, 0, d[3]);
        }
    }

    pub fn release(mut self) {
        for group in self.draws {
            release(group);
        }
        self.mark.release();
    }
}
