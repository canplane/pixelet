// pxl-render/gpu/pass/impostor.rs

//! Decor's impostor tier: each variant drawn once from the side into an atlas
//! layer, redrawn only when its model or skin changes, and drawn per instance as
//! a quad turned to the eye.

use crate::frame::draw::uniform_of;
use crate::frame::draw::{DRAW_WORDS, PIPELINE_IMPOSTOR};
use crate::frame::shader::record::PIPELINE_IMPOSTOR_BAKE;
use xpute_gpu::enums::{AddressMode, FilterMode, MipmapFilterMode};

use crate::frame::shader::word::DECOR_GROUPS;
use crate::gpu::api::{
    buf_range_entry, buffer_usage, bytes, pass_desc, release, sampler_entry, texture_usage,
    view_entry, Format, Gpu, Load, TrackedRenderPass,
};
use crate::gpu::generated::Drawn;
use crate::gpu::pass::decor::DecorPass;
use crate::gpu::pass::model::{
    draw_model, draw_models, put_instances, DrawUniforms, ModelDraw, DRAW_BYTES,
};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::upload::Uploads;

/// Impostors span less than the model tier's floor, a camera grid cell
/// (pxl-spec-core `grid::CELLS_A_RAD`), 45 drawn pixels on a view 2160
/// CSS pixels high at ratio 2, so this is several times any one's drawn
/// size.
const IMPOSTOR_SIZE: u32 = 128;
const FIT_BYTES: u32 = 16;

/// glTF's across, up and deep.
pub type Extent = [f32; 3];

type Baked = (u32, Option<u32>, Extent);

pub struct ImpostorPass {
    draw: Built,
    bake: Built,
    uniforms: DrawUniforms,
    fit_uniforms: DrawUniforms,
    atlas: u32,
    depth_view: u32,
    fits: u32,
    group: u32,
    /// Center across, center up, half side, half depth.
    fit: [[f32; 4]; DECOR_GROUPS],
    baked: [Option<Baked>; DECOR_GROUPS],
    wanted: [Option<Baked>; DECOR_GROUPS],
}

/// A little room around it so a filtered edge reads nothing past it.
fn fit_of(extent: Extent) -> [f32; 4] {
    let [x, y, z] = extent;
    let half = 0.5 * x.max(y) * 1.05;
    [0.0, 0.5 * y, half.max(1e-3), (0.5 * z * 1.05).max(1e-3)]
}

impl ImpostorPass {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines, align: u32) -> ImpostorPass {
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
            uniforms: DrawUniforms::new(DRAW_BYTES, align),
            fit_uniforms: DrawUniforms::new(FIT_BYTES, align),
            atlas,
            depth_view,
            fits,
            group,
            fit: [[0.0; 4]; DECOR_GROUPS],
            baked: [None; DECOR_GROUPS],
            wanted: [None; DECOR_GROUPS],
        }
    }

    pub fn set_models(&mut self, groups: impl Iterator<Item = (u32, Option<u32>, Extent)>) {
        for (group, want) in groups.take(DECOR_GROUPS).enumerate() {
            self.wanted[group] = Some(want);
        }
    }

    /// Whether a group was baked again.
    pub fn bake(&mut self, g: &Gpu, encoder: u32, decor: &DecorPass, uploads: &Uploads) -> bool {
        // The groups whose model or extent moved since baked, each once, and
        // whose model has landed: one still landing is due on a later frame,
        // and this one bakes, writes and submits nothing for it.
        let (mut due, mut len) = ([(0, Baked::default(), (0, 0, 0, 0)); DECOR_GROUPS], 0);
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
            pass.set_pipeline(self.bake.pipeline);
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

    /// Every upload group the draws will bind is made here, so drawing makes
    /// nothing that outlives its frame.
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
            self.draw.layouts[1],
            draws.len() as u32,
            draws.clone(),
            moved,
            |buf, d| {
                put_instances(buf, uniform_of(uniforms, d), d[1]);
            },
        );
        for d in draws {
            uploads.group(g, d[0]);
        }
    }

    /// A group whose layer is not drawn yet is not drawn.
    pub fn draw<'a>(
        &self,
        pass: &mut TrackedRenderPass,
        draws: impl Iterator<Item = &'a [u32; DRAW_WORDS]>,
        uploads: &mut Uploads,
        frame: u32,
    ) {
        let Some(slots) = self.uniforms.slots() else {
            return;
        };
        draw_models(
            pass,
            &self.draw,
            self.draw.pipeline,
            frame,
            slots,
            draws,
            |d| {
                self.baked.get(d[1] as usize).copied().flatten()?;
                Some(ModelDraw {
                    groups: [self.group, uploads.group_of(d[0])?],
                    mesh: None,
                    instances: d[2],
                })
            },
        );
    }

    /// The impostors the GPU generated, an indirect draw a group whose layer
    /// is drawn (pxl-spec-core `scatter::drawn`, a group's impostors its region
    /// `2·group + 1`).
    pub fn draw_generated(
        &self,
        pass: &mut TrackedRenderPass,
        generated: Option<Drawn>,
        groups: u32,
        frame: u32,
    ) {
        let Some(d) = generated else { return };
        pass.set_pipeline(self.draw.pipeline);
        pass.set_bind_group(0, frame, &[]);
        pass.set_bind_group(2, self.group, &[]);
        pass.set_bind_group(3, d.instances_group, &[]);
        for group in 0..groups.min(DECOR_GROUPS as u32) {
            if self.baked[group as usize].is_none() {
                continue;
            }
            pass.set_bind_group(1, d.uniform_group, &[d.uniform_at(2 * group + 1)]);
            pass.draw_indirect(d.args, d.args_at(2 * group + 1));
        }
    }
}
