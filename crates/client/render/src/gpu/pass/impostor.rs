// pxl-render/gpu/pass/impostor.rs

//! Decor's impostor tier: each variant drawn once from the side into an atlas
//! layer, redrawn only when its model or skin changes, and drawn per instance as
//! a quad turned to the eye.

use crate::frame::draw::uniform_of;
use crate::frame::draw::{DRAW_WORDS, PIPELINE_IMPOSTOR};
use crate::frame::shader::record::PIPELINE_IMPOSTOR_BAKE;
use xpute_gpu::enums::{AddressMode, FilterMode, MipmapFilterMode};

use crate::gpu::api::{buffer_range_entry, buffer_usage, bytes, pass_desc, release, sampler_entry, texture_usage, view_entry, Format, Gpu, Load};
use crate::gpu::grown::Drawn;
use crate::gpu::pass::decor::{DecorPass, GROUPS};
use crate::gpu::pass::model::{draw_model, draw_models, put_instances, DrawUniforms, ModelDraw, DRAW_BYTES};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::renderer::Blocks;

/// Impostors cover fewer pixels than the model tier's floor (twelve at the
/// reference viewport), so this is several times any one's drawn size.
const SPRITE: u32 = 128;
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
    fit: [[f32; 4]; GROUPS],
    baked: [Option<Baked>; GROUPS],
    wanted: [Option<Baked>; GROUPS],
}

/// A little room around it so a filtered edge reads nothing past it.
fn fit_of(extent: Extent) -> [f32; 4] {
    let [x, y, z] = extent;
    let half = 0.5 * x.max(y) * 1.05;
    [0.0, 0.5 * y, half.max(1e-3), (0.5 * z * 1.05).max(1e-3)]
}

impl ImpostorPass {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines, align: u32) -> ImpostorPass {
        let draw = pipelines.build(g, PIPELINE_IMPOSTOR, g.canvas());
        let bake = pipelines.build(g, PIPELINE_IMPOSTOR_BAKE, Format::rgba8unorm_srgb);
        let sampler = g.sampler(FilterMode::linear, FilterMode::linear, MipmapFilterMode::linear, AddressMode::clamp_to_edge, None, 1);
        let atlas = g.texture(
            SPRITE,
            SPRITE,
            GROUPS as u32,
            Format::rgba8unorm_srgb,
            1,
            texture_usage::RENDER_ATTACHMENT | texture_usage::TEXTURE_BINDING,
        );
        let atlas_view = g.view(atlas, true, 0, 1, 0, GROUPS as u32);
        let depth = g.texture(SPRITE, SPRITE, 1, Format::depth32float, 1, texture_usage::RENDER_ATTACHMENT);
        let depth_view = g.view(depth, false, 0, 1, 0, 1);
        let fits = g.buffer((GROUPS * 16) as u32, buffer_usage::UNIFORM | buffer_usage::COPY_DST);
        let group = g.bind_group(
            draw.layouts[2],
            &[view_entry(0, atlas_view), sampler_entry(1, sampler), buffer_range_entry(2, fits, 0, (GROUPS * 16) as u32)],
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
            fit: [[0.0; 4]; GROUPS],
            baked: [None; GROUPS],
            wanted: [None; GROUPS],
        }
    }

    pub fn set_models(&mut self, groups: impl Iterator<Item = (u32, Option<u32>, Extent)>) {
        for (group, want) in groups.take(GROUPS).enumerate() {
            self.wanted[group] = Some(want);
        }
    }

    /// Whether a group was baked again.
    pub fn bake(&mut self, g: &Gpu, encoder: u32, decor: &DecorPass, blocks: &Blocks) -> bool {
        // The groups whose model or extent moved since baked, each once, and
        // whose model has landed: one still landing is due on a later frame,
        // and this one bakes, writes and submits nothing for it.
        let (mut due, mut len) = ([(0, Baked::default(), (0, 0, 0, 0)); GROUPS], 0);
        for k in 0..GROUPS {
            if let Some(w) = self.wanted[k].filter(|w| w.0 != 0 && w.1.is_some() && self.baked[k] != Some(*w)) {
                if let Some(model) = decor.model_of(g, k as u32, blocks) {
                    (due[len], len) = ((k, w, model), len + 1);
                }
            }
        }
        let due = &due[..len];
        if due.is_empty() {
            return false;
        }
        self.fit_uniforms.write(g, self.bake.layouts[0], due.len() as u32, due.iter(), true, |buf, &(_, w, _)| {
            buf[..FIT_BYTES as usize].copy_from_slice(bytes(&fit_of(w.2)));
        });
        let Some(slots) = self.fit_uniforms.slots() else { return false };
        for (n, &(k, w, (model, vertex_cnt, index_cnt, skin))) in due.iter().enumerate() {
            let (fit_group, offset) = slots.at(n as u32);
            let layer = g.view(self.atlas, false, 0, 1, k as u32, 1);
            let pass = g.pass(encoder, &pass_desc(layer, Load::Clear, self.depth_view, Load::Clear, 1.0, None));
            g.set_pipeline(pass, self.bake.pipeline);
            g.set_bind_group(pass, 0, fit_group, &[offset]);
            g.set_bind_group(pass, 1, skin, &[]);
            draw_model(g, pass, &self.bake, &mut 0, model, vertex_cnt, index_cnt, 1);
            g.end(pass);
            release(layer);
            self.fit[k] = fit_of(w.2);
            self.baked[k] = Some(w);
        }
        g.write_buffer(self.fits, 0, bytes(self.fit.as_flattened()));
        true
    }

    /// Every block group the draws will bind is made here, so drawing makes
    /// nothing that outlives its frame.
    /// `moved` whether the lists the draws are made from moved.
    pub fn prepare<'a>(&mut self, g: &Gpu, draws: impl ExactSizeIterator<Item = &'a [u32; DRAW_WORDS]> + Clone, uniforms: &[f32], moved: bool, blocks: &mut Blocks) {
        self.uniforms.write(g, self.draw.layouts[1], draws.len() as u32, draws.clone(), moved, |buf, d| {
            put_instances(buf, uniform_of(uniforms, d), d[1]);
        });
        for d in draws {
            blocks.group(g, d[0], self.draw.layouts[3]);
        }
    }

    /// A group whose layer is not drawn yet is not drawn.
    pub fn draw<'a>(&self, g: &Gpu, pass: u32, draws: impl Iterator<Item = &'a [u32; DRAW_WORDS]>, blocks: &mut Blocks, frame: (u32, &mut u32)) {
        let Some(slots) = self.uniforms.slots() else { return };
        let block_layout = self.draw.layouts[3];
        draw_models(g, pass, &self.draw, self.draw.pipeline, frame, slots, 0, draws, |d| {
            self.baked.get(d[1] as usize).copied().flatten()?;
            Some(ModelDraw {
                groups: [self.group, blocks.group(g, d[0], block_layout)?],
                mesh: None,
                instances: d[2],
            })
        });
    }

    /// The sprites the GPU grew, an indirect draw a group whose layer is
    /// drawn (pxl-spec-core `scatter::drawn`, a group's sprites its region
    /// `2·group + 1`).
    pub fn draw_grown(&self, g: &Gpu, pass: u32, grown: Option<Drawn>, groups: u32, (frame, bound): (u32, &mut u32)) {
        let Some(d) = grown else { return };
        g.set_pipeline(pass, self.draw.pipeline);
        if *bound != frame {
            g.set_bind_group(pass, 0, frame, &[]);
            *bound = frame;
        }
        g.set_bind_group(pass, 2, self.group, &[]);
        g.set_bind_group(pass, 3, d.block_group, &[]);
        for group in 0..groups.min(GROUPS as u32) {
            if self.baked[group as usize].is_none() {
                continue;
            }
            g.set_bind_group(pass, 1, d.uniform_group, &[d.uniform_at(2 * group + 1)]);
            g.draw_indirect(pass, d.args, d.args_at(2 * group + 1));
        }
    }
}
