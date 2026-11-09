// pxl-render/pass/impostor.rs

//! The decor's impostor tier. Each draw group's model is baked once, seen from
//! the side, into a layer of an atlas, and baked again only when its model,
//! skin or extent changes. Each instance is drawn as a quad turned to the eye
//! about the world's up and textured from its group's layer. A listed draw
//! that changed marks the screen's tiles it covers where it was and where it
//! is (pass/mark.rs). The upload such a draw reads holds its instances.

use crate::frame::shader::record::PIPELINE_IMPOSTOR;
use crate::frame::shader::record::{PIPELINE_IMPOSTOR_BAKE, PIPELINE_IMPOSTOR_MARK};
use crate::gpu::draw::DRAW_WORDS;
use crate::gpu::draw::{uniform_of, DrawStat, DrawTable};
use xpute_gpu::enums::{AddressMode, FilterMode, MipmapFilterMode};

use crate::frame::shader::sequence;
use crate::frame::shader::word::DECOR_GROUPS;
use crate::gpu::api::{
    buf_range_entry, buffer_usage, bytes, pass_desc, release, sampler_entry, texture_usage,
    view_entry, Format, Gpu, Load, TrackedRenderPass,
};
use crate::gpu::generator::IndirectDraw;
use crate::gpu::pipeline::{Pipeline, PipelineCache};
use crate::gpu::upload::UploadTable;
use crate::pass::decor::DecorPass;
use crate::pass::mark::{put_whole, MarkPass};
use crate::pass::model::{
    draw_model, draw_models, put_uniform, Extent, ModelDraw, UniformBuffer, DRAW_BYTES,
};

/// The side of an atlas layer, in texels. An impostor is drawn only smaller
/// than the model tier's floor, which is one camera grid cell (pxl-engine
/// `grid::CELLS_A_RAD`). A cell is 45 drawn pixels on a view 2160 CSS pixels
/// high at a pixel ratio of 2, so this side is several times any impostor's
/// drawn size.
const IMPOSTOR_SIZE: u32 = 128;
const FIT_BYTES: u32 = 16;

type BakeKey = (u32, Option<u32>, Extent);

pub struct ImpostorPass {
    draw: Pipeline,
    bake: Pipeline,
    uniforms: UniformBuffer,
    fit_uniforms: UniformBuffer,
    atlas: u32,
    depth_view: u32,
    fits: u32,
    group: u32,
    /// Each group's fit in its layer, as impostor_place.wgsl and
    /// impostor_bake.wgsl read it: the center across, the center up, the half
    /// side and the half depth, in the model's units.
    fit: [[f32; 4]; DECOR_GROUPS],
    baked: [Option<BakeKey>; DECOR_GROUPS],
    wanted: [Option<BakeKey>; DECOR_GROUPS],
    pub mark: MarkPass,
}

/// The fit in its layer of a model of size `extent` (`ImpostorPass::fit`).
/// It leaves a little room around the model, so a filtered edge reads
/// nothing past it.
fn fit_of(extent: Extent) -> [f32; 4] {
    let [across, deep, up] = extent;
    let half = 0.5 * across.max(up) * 1.05;
    [0.0, 0.5 * up, half.max(1e-3), (0.5 * deep * 1.05).max(1e-3)]
}

impl ImpostorPass {
    pub fn new(g: &Gpu, pipelines: &mut PipelineCache, align: u32) -> ImpostorPass {
        let draw = pipelines.build_world(g, PIPELINE_IMPOSTOR);
        let bake = pipelines.build(g, PIPELINE_IMPOSTOR_BAKE, Format::rgba8unorm_srgb);
        let sampler = g.sampler(
            FilterMode::linear,
            FilterMode::linear,
            MipmapFilterMode::linear,
            AddressMode::clamp_to_edge,
            None,
            1,
        );
        let atlas = g.texture(
            IMPOSTOR_SIZE,
            IMPOSTOR_SIZE,
            DECOR_GROUPS as u32,
            Format::rgba8unorm_srgb,
            1,
            texture_usage::RENDER_ATTACHMENT | texture_usage::TEXTURE_BINDING,
        );
        let atlas_view = g.view(atlas, true, 0, 1, 0, DECOR_GROUPS as u32);
        let depth = g.texture(
            IMPOSTOR_SIZE,
            IMPOSTOR_SIZE,
            1,
            Format::depth32float,
            1,
            texture_usage::RENDER_ATTACHMENT,
        );
        let depth_view = g.view(depth, false, 0, 1, 0, 1);
        let fits = g.buf(
            DECOR_GROUPS as u32 * FIT_BYTES,
            buffer_usage::UNIFORM | buffer_usage::COPY_DST,
        );
        let group = g.bind_group(
            draw.layouts[2],
            &[
                view_entry(0, atlas_view),
                sampler_entry(1, sampler),
                buf_range_entry(2, fits, 0, DECOR_GROUPS as u32 * FIT_BYTES),
            ],
        );
        ImpostorPass {
            draw,
            bake,
            uniforms: UniformBuffer::new(DRAW_BYTES, align),
            fit_uniforms: UniformBuffer::new(FIT_BYTES, align),
            atlas,
            depth_view,
            fits,
            group,
            fit: [[0.0; 4]; DECOR_GROUPS],
            baked: [None; DECOR_GROUPS],
            wanted: [None; DECOR_GROUPS],
            mark: MarkPass::new(
                g,
                pipelines,
                (PIPELINE_IMPOSTOR_MARK, PIPELINE_IMPOSTOR),
                align,
                (|d| d[0], put_whole),
            ),
        }
    }

    /// Draws this frame's marks into `pass`. `frames` holds their group 0 as
    /// they are and as they were (tile_mark.wgsl, `MarkPass::draw`). A group
    /// whose layer is not baked yet draws no mark.
    pub fn draw_marks(&self, pass: &mut TrackedRenderPass, frames: [u32; 2]) {
        self.mark.draw(pass, frames, |pass, d, upload| {
            if self.baked.get(d[1] as usize).copied().flatten().is_none() {
                return;
            }
            pass.set_bind_group(2, self.group, &[]);
            pass.set_bind_group(3, upload, &[]);
            pass.draw(4, d[2], 0, 0);
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

    /// Adds the pass's draws to `stat`, two triangles an instance.
    pub fn count(&self, draws: &DrawTable, stat: &mut DrawStat) {
        draws.count(PIPELINE_IMPOSTOR, stat, |d| 2 * d[2]);
    }

    pub fn set_models(&mut self, groups: impl Iterator<Item = (u32, Option<u32>, Extent)>) {
        for (group, want) in groups.take(DECOR_GROUPS).enumerate() {
            self.wanted[group] = Some(want);
        }
    }

    /// Bakes again each group that is due, and says whether any was.
    pub fn bake(
        &mut self,
        g: &Gpu,
        encoder: u32,
        decor: &DecorPass,
        uploads: &UploadTable,
    ) -> bool {
        // A group is due when its model, skin or extent changed since it was
        // baked and its model has landed. A group still landing is due on a
        // later frame, and this frame bakes, writes and submits nothing for
        // it.
        let (mut due, mut len) = ([(0, BakeKey::default(), (0, 0, 0, 0)); DECOR_GROUPS], 0);
        for k in 0..DECOR_GROUPS {
            if let Some(w) =
                self.wanted[k].filter(|w| w.0 != 0 && w.1.is_some() && self.baked[k] != Some(*w))
            {
                if let Some(model) = decor.model_of(g, k as u32, uploads) {
                    (due[len], len) = ((k, w, model), len + 1);
                }
            }
        }
        let due = &due[..len];
        if due.is_empty() {
            return false;
        }
        self.fit_uniforms.write(
            g,
            self.bake.layouts[0],
            due.len() as u32,
            due.iter(),
            true,
            |buf, &(_, w, _)| {
                buf[..FIT_BYTES as usize].copy_from_slice(bytes(&fit_of(w.2)));
            },
        );
        let Some(slots) = self.fit_uniforms.slots() else {
            return false;
        };
        for (n, &(k, w, (model, vertex_cnt, idx_cnt, skin))) in due.iter().enumerate() {
            let (fit_group, off) = slots.at(n as u32);
            let layer = g.view(self.atlas, false, 0, 1, k as u32, 1);
            let pass = g.pass(
                encoder,
                &pass_desc(layer, Load::Clear, self.depth_view, Load::Clear, 1.0, None),
            );
            let mut pass = TrackedRenderPass::new(g, pass);
            pass.set_pipeline(self.bake.id);
            pass.set_bind_group(0, fit_group, &[off]);
            pass.set_bind_group(1, skin, &[]);
            draw_model(&mut pass, &self.bake, model, vertex_cnt, idx_cnt, 1);
            pass.end();
            release(layer);
            self.fit[k] = fit_of(w.2);
            self.baked[k] = Some(w);
        }
        g.write_buf(self.fits, 0, bytes(self.fit.as_flattened()));
        true
    }

    /// Writes the draws' uniforms and makes every upload group the draws will
    /// bind, so that drawing makes none: a group made inside a frame that is
    /// dropped would be gone. `changed` says whether the lists the draws are
    /// made from moved.
    pub fn prepare<'a>(
        &mut self,
        g: &Gpu,
        draws: impl ExactSizeIterator<Item = &'a [u32; DRAW_WORDS]> + Clone,
        uniforms: &[f32],
        changed: bool,
        uploads: &mut UploadTable,
    ) {
        self.uniforms.write(
            g,
            self.draw.layouts[1],
            draws.len() as u32,
            draws.clone(),
            changed,
            |buf, d| {
                put_uniform(buf, uniform_of(uniforms, d));
            },
        );
        for d in draws {
            uploads.group(g, d[0]);
        }
    }

    /// Draws the listed impostors. A group whose layer is not baked yet is
    /// not drawn.
    pub fn draw<'a>(
        &self,
        pass: &mut TrackedRenderPass,
        draws: impl Iterator<Item = &'a [u32; DRAW_WORDS]>,
        uploads: &mut UploadTable,
        frame: u32,
    ) {
        let Some(slots) = self.uniforms.slots() else {
            return;
        };
        draw_models(pass, &self.draw, self.draw.id, frame, slots, draws, |d| {
            self.baked.get(d[1] as usize).copied().flatten()?;
            Some(ModelDraw {
                groups: [self.group, uploads.group_of(d[0])?],
                mesh: None,
                instance_cnt: d[2],
            })
        });
    }

    /// Draws the impostors the GPU generated, one indirect draw for each group
    /// whose layer is baked. A group's impostors are in region `2·group + 1`
    /// of what the GPU generated (pxl-content `generation::drawn`).
    pub fn draw_generated(
        &self,
        pass: &mut TrackedRenderPass,
        generated: Option<IndirectDraw>,
        groups: u32,
        frame: u32,
    ) {
        let Some(d) = generated else { return };
        let step = if d.mask.is_none() {
            &sequence::IMPOSTOR_LIT
        } else {
            &sequence::IMPOSTOR_LIT_MASKED
        };
        pass.set_pipeline(self.draw.id);
        pass.set_bind_group(0, frame, &[]);
        pass.set_bind_group(2, self.group, &[]);
        pass.set_bind_group(3, d.instances_group, &[]);
        for group in 0..groups.min(DECOR_GROUPS as u32) {
            if self.baked[group as usize].is_none() {
                continue;
            }
            pass.set_bind_group(1, d.uniform_group, &[d.uniform_at(2 * group + 1)]);
            pass.draw_indirect(d.args, step.off(d.args_at(2 * group + 1)));
        }
    }
}
