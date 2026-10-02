// pxl-render/gpu/pass/impostor.rs

//! Decor's impostor tier: each variant drawn once from the side into an atlas
//! layer, redrawn only when its model or skin changes, and drawn per instance as
//! a quad turned to the eye.

use crate::frame::draw::{DRAW_WORDS, PIPELINE_IMPOSTOR};
use crate::frame::shader::record::PIPELINE_IMPOSTOR_BAKE;
use crate::gpu::api::{buffer_usage, bytes, object, pass_desc, release, texture_usage, Gpu, Stamp};
use crate::gpu::api_spec::{address, compare, dimension, filter, format, load, resource};
use crate::gpu::pass::decor::{DecorPass, GROUPS};
use crate::gpu::pass::model::{draw_model, draw_models, put_instances, uniform_of, DrawUniforms, ModelDraw};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::renderer::Blocks;

/// Impostors cover fewer pixels than the model tier's floor (twelve at the
/// reference viewport), so this is several times any one's drawn size.
const SPRITE: u32 = 128;
/// Translation, first lane word, stride, opacity, layer, index list's first word.
const DRAW_UNIFORM_BYTES: u32 = 32;
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
        let draw = pipelines.build(g, PIPELINE_IMPOSTOR, format::CANVAS);
        let bake = pipelines.build(g, PIPELINE_IMPOSTOR_BAKE, format::RGBA8_SRGB);
        let sampler = object();
        g.sampler(sampler, filter::LINEAR, filter::LINEAR, filter::LINEAR, address::CLAMP, compare::NONE, 1);
        let atlas = object();
        g.texture(
            atlas,
            SPRITE,
            SPRITE,
            GROUPS as u32,
            format::RGBA8_SRGB,
            1,
            texture_usage::RENDER_ATTACHMENT | texture_usage::TEXTURE_BINDING,
        );
        let atlas_view = object();
        g.view(atlas_view, atlas, dimension::D2_ARRAY, 0, 1, 0, GROUPS as u32);
        let depth = object();
        g.texture(depth, SPRITE, SPRITE, 1, format::DEPTH32, 1, texture_usage::RENDER_ATTACHMENT);
        let depth_view = object();
        g.view(depth_view, depth, dimension::D2, 0, 1, 0, 1);
        let fits = object();
        g.buffer(fits, (GROUPS * 16) as u32, buffer_usage::UNIFORM | buffer_usage::COPY_DST);
        let group = object();
        g.bind_group(
            group,
            draw.layouts[2],
            &[
                0,
                resource::VIEW,
                atlas_view,
                0,
                0,
                1,
                resource::SAMPLER,
                sampler,
                0,
                0,
                2,
                resource::BUFFER,
                fits,
                0,
                (GROUPS * 16) as u32,
            ],
        );
        ImpostorPass {
            draw,
            bake,
            uniforms: DrawUniforms::new(DRAW_UNIFORM_BYTES, align),
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

    pub fn set_models(&mut self, groups: &[(u32, Option<u32>, Extent)]) {
        for (group, &want) in groups.iter().take(GROUPS).enumerate() {
            self.wanted[group] = Some(want);
        }
    }

    pub fn bake(&mut self, g: &Gpu, encoder: u32, decor: &mut DecorPass, blocks: &Blocks) {
        let due: Vec<(usize, Baked)> = (0..GROUPS)
            .filter_map(|k| self.wanted[k].filter(|w| w.0 != 0 && w.1.is_some() && self.baked[k] != Some(*w)).map(|w| (k, w)))
            .collect();
        if due.is_empty() {
            return;
        }
        self.fit_uniforms.write(g, self.bake.layouts[0], due.len() as u32, due.iter(), |buf, &(_, w)| {
            buf[..FIT_BYTES as usize].copy_from_slice(bytes(&fit_of(w.2)));
        });
        let Some(slots) = self.fit_uniforms.slots() else { return };
        for (n, &(k, w)) in due.iter().enumerate() {
            let Some((model, vertex_cnt, index_cnt, skin)) = decor.model_of(g, k as u32, blocks) else {
                continue;
            };
            let (fit_group, offset) = slots.at(n as u32);
            let layer = object();
            g.view(layer, self.atlas, dimension::D2, 0, 1, k as u32, 1);
            let pass = object();
            g.pass(pass, encoder, &pass_desc(layer, load::CLEAR, self.depth_view, load::CLEAR, 1.0, Stamp::UNTIMED));
            g.set_pipeline(pass, self.bake.pipeline);
            g.set_bind_group(pass, 0, fit_group, &[offset]);
            g.set_bind_group(pass, 1, skin, &[]);
            draw_model(g, pass, &self.bake, &mut 0, model, vertex_cnt, index_cnt, 1);
            g.end(pass);
            release(pass);
            release(layer);
            self.fit[k] = fit_of(w.2);
            self.baked[k] = Some(w);
        }
        g.write_buffer(self.fits, 0, bytes(self.fit.as_flattened()));
    }

    pub fn prepare<'a>(&mut self, g: &Gpu, draws: impl ExactSizeIterator<Item = &'a [u32; DRAW_WORDS]>, uniforms: &[f32]) {
        self.uniforms.write(g, self.draw.layouts[1], draws.len() as u32, draws, |buf, d| {
            put_instances(buf, uniform_of(uniforms, d), d[1]);
        });
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
}
