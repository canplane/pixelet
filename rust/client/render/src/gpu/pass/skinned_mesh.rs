// pxl-render/gpu/pass/skinned_mesh.rs

//! Characters: each model's mesh upload skinned by each character's joint palette. A
//! case of the variant picker draws one character in a pass of its own. A character's
//! vertices read its draw off the list's uniforms at its slot. A character whose
//! draw changed marks the tiles it covers where it was and where it is
//! (gpu/pass/mark.rs), its palette the upload it reads.

use crate::frame::shader::record::PIPELINE_SKINNED;
use crate::frame::shader::record::{PIPELINE_SKINNED_DEPTH, PIPELINE_SKINNED_MARK};
use crate::gpu::draw::{DrawStats, Draws, DRAW_WORDS};

use crate::gpu::api::{buf_entry, release, Format, Gpu, Image, Layout, Stencil, TrackedRenderPass};
use crate::gpu::pass::mark::{put_whole, Marks};
use crate::gpu::pass::model::{bind_mesh, draw_model, skin_group, skin_made, Stage};
use crate::gpu::pipeline::{Pipeline, Pipelines};
use crate::gpu::upload::Uploads;

pub struct SkinnedPass {
    lit: Pipeline,
    depth: Pipeline,
    /// The list's uniforms as the lit pass and the depth pass bind them, and
    /// their buffer (`set_draws`).
    draws: [u32; 2],
    draws_buf: u32,
    /// Each model's skin image, as the frame hands them.
    skins: Vec<(u32, Image)>,
    pub marks: Marks,
}

impl SkinnedPass {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines, align: u32) -> SkinnedPass {
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
            marks: Marks::new(
                g,
                pipelines,
                (PIPELINE_SKINNED_MARK, PIPELINE_SKINNED),
                align,
                (|d| d[1], put_whole, |_| true),
            ),
        }
    }

    /// Group 0's layout of the marks (tile_mark.wgsl).
    pub fn mark_layout(&self) -> Layout {
        self.marks.layout()
    }

    /// This frame's marks drawn into `pass`, `frames` their group
    /// 0 as they are and as they were: the eye's or the light's frame and its
    /// tiles' (tile_mark.wgsl, `Marks::draw`).
    pub fn draw_marks(&self, pass: &mut TrackedRenderPass, frames: [u32; 2], uploads: &Uploads) {
        self.marks.draw(pass, frames, |pass, d, palette| {
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

    /// Its draws into `stats`, its model's triangles each, 0 while it loads.
    pub fn count(&self, draws: &Draws, uploads: &Uploads, stats: &mut DrawStats) {
        draws.count(PIPELINE_SKINNED, stats, |d| {
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

    /// The list's uniforms, the buffer `buf`, as the lit pass and the depth
    /// pass bind them in group 1, made again only where the buffer is
    /// another; before the frame's commands, as a group made in a frame
    /// dropped would be gone.
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

    /// Every palette and skin group the draws will bind is made here, so
    /// drawing makes nothing that outlives its frame; a skin whose image
    /// lands later is drawn on a later frame.
    pub fn prepare<'a>(
        &mut self,
        g: &Gpu,
        draws: impl Iterator<Item = &'a [u32; DRAW_WORDS]>,
        uploads: &mut Uploads,
    ) {
        for d in draws {
            uploads.group(g, d[1]);
            if let Some(image) = self.image_of(d[2]) {
                skin_group(g, self.lit.layouts[2], image);
            }
        }
    }

    /// Lit, or their depth from the sun (`Stage::Depth`), `frame` the group
    /// 0 the stage reads: the list's uniforms bound once (`set_draws`), and
    /// each character drawn as the instance its slot names, which its vertices
    /// read their draw at (skinned_draw.slot.wgsl), with its own mesh, skin and
    /// palette.
    pub fn draw<'a>(
        &self,
        pass: &mut TrackedRenderPass,
        draws: impl Iterator<Item = &'a [u32; DRAW_WORDS]>,
        uploads: &mut Uploads,
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
        self.marks.release();
    }
}
