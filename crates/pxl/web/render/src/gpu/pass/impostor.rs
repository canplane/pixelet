// pxl-render/gpu/pass/impostor.rs

//! Decor's impostor tier: each variant drawn once, from the side, into its
//! layer of a sprite atlas, and every instance the scheduler lists as an
//! impostor drawn as a quad turned to the eye with that sprite on it — two
//! triangles where the model is thousands, for a variant too small on the
//! screen for its model to be worth them (kernel layer/decor/build.rs
//! `decor_tier`). The instances are the decor block's, read as the decor
//! pass reads them; the sprite is the model's own, so it is made from what
//! the variant is and from nothing chosen per kind.
//!
//! A layer is drawn once its model and skin are on the device and drawn
//! again only when either changes. The fit is the variant's extent: its
//! side, foot at the bottom, in a square; the quad is that square at the
//! instance's scale.

use crate::frame::draw::{DRAW_WORDS, PIPELINE_IMPOSTOR, UNIFORM_FLOATS};
use crate::frame::shader::record::PIPELINE_IMPOSTOR_BAKE;
use crate::gpu::api::{buffer_usage, object, pass_desc, release, texture_usage, Gpu, Stamp};
use crate::gpu::api_spec::{address, compare, dimension, filter, format, load, resource};
use crate::gpu::atmosphere::AIR_WORDS;
use crate::gpu::pass::decor::{DecorPass, GROUPS};
use crate::gpu::pass::model::{draw_model, DrawUniforms, FrameGroups};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::renderer::{put_f32, put_u32, Blocks};
use crate::gpu::shadow::Shadow;

/// A sprite's side in texels: a variant drawn as an impostor covers fewer
/// pixels than the model tier's floor, twelve at the reference viewport, so
/// this is several times what any one is drawn at.
const SPRITE: u32 = 128;
/// An impostor draw's dynamic uniform (shader/impostor.wgsl's words):
/// translation, the group's first lane word, the count, the opacity, the
/// layer, padding.
const DRAW_UNIFORM_BYTES: u32 = 32;
/// A bake's dynamic uniform: the fit.
const FIT_BYTES: u32 = 16;

/// A variant's extent as the kernel has it, glTF's: across, up, and deep.
pub type Extent = [f32; 3];

/// What a layer was drawn from: the model's block, its skin's image, its
/// extent.
type Baked = (u32, Option<u32>, Extent);

pub struct ImpostorPass {
    draw: Built,
    bake: Built,
    frames: FrameGroups,
    sampler: u32,
    uniforms: DrawUniforms,
    fit_uniforms: DrawUniforms,
    atlas: u32,
    atlas_view: u32,
    depth: u32,
    depth_view: u32,
    fits: u32,
    group: u32,
    /// Each layer's fit, as the draw reads it: center across, center up,
    /// half side, half depth.
    fit: [[f32; 4]; GROUPS],
    /// What each layer holds now; none until it is drawn.
    baked: [Option<Baked>; GROUPS],
    /// What each group is now.
    wanted: [Option<Baked>; GROUPS],
}

/// A variant's fit in its square: its side centered across, its foot at the
/// bottom, a little room around it so a filtered edge reads nothing past it.
fn fit_of(extent: Extent) -> [f32; 4] {
    let [x, y, z] = extent;
    let half = 0.5 * x.max(y) * 1.05;
    [0.0, 0.5 * y, half.max(1e-3), (0.5 * z * 1.05).max(1e-3)]
}

impl ImpostorPass {
    #[allow(clippy::too_many_arguments)]
    pub fn new(g: &Gpu, pipelines: &mut Pipelines, frame_buffer: u32, shadow: &Shadow, ao: [u32; 5], air: [u32; AIR_WORDS], align: u32) -> ImpostorPass {
        let draw = pipelines.build(g, PIPELINE_IMPOSTOR, format::CANVAS);
        let bake = pipelines.build(g, PIPELINE_IMPOSTOR_BAKE, format::RGBA8_SRGB);
        let sampler = object();
        g.sampler(sampler, filter::LINEAR, filter::LINEAR, filter::LINEAR, address::CLAMP, compare::NONE, 1);
        let atlas = object();
        g.texture(atlas, SPRITE, SPRITE, GROUPS as u32, format::RGBA8_SRGB, 1, texture_usage::RENDER_ATTACHMENT | texture_usage::TEXTURE_BINDING);
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
            &[0, resource::VIEW, atlas_view, 0, 0, 1, resource::SAMPLER, sampler, 0, 0, 2, resource::BUFFER, fits, 0, (GROUPS * 16) as u32],
        );
        ImpostorPass {
            frames: FrameGroups::new(g, draw.layouts[0], frame_buffer, shadow.entries(1), None, ao, air),
            draw,
            bake,
            sampler,
            uniforms: DrawUniforms::new(DRAW_UNIFORM_BYTES, align),
            fit_uniforms: DrawUniforms::new(FIT_BYTES, align),
            atlas,
            atlas_view,
            depth,
            depth_view,
            fits,
            group,
            fit: [[0.0; 4]; GROUPS],
            baked: [None; GROUPS],
            wanted: [None; GROUPS],
        }
    }

    /// Group 0 again over a new occlusion map's entry `ao`.
    pub fn rebind(&mut self, g: &Gpu, frame_buffer: u32, shadow: &Shadow, ao: [u32; 5]) {
        self.frames.rebind(g, self.draw.layouts[0], frame_buffer, shadow.entries(1), None, ao);
    }

    /// Each group's model block, skin image and extent as the kernel has them
    /// now, in group order.
    pub fn set_models(&mut self, groups: &[(u32, Option<u32>, Extent)]) {
        for (group, &want) in groups.iter().take(GROUPS).enumerate() {
            self.wanted[group] = Some(want);
        }
    }

    /// Draws every layer whose model or skin changed and is on the device
    /// now, before the frame's passes read the atlas.
    pub fn bake(&mut self, g: &Gpu, encoder: u32, decor: &mut DecorPass, blocks: &Blocks) {
        let due: Vec<(usize, Baked)> =
            (0..GROUPS).filter_map(|k| self.wanted[k].filter(|w| w.0 != 0 && w.1.is_some() && self.baked[k] != Some(*w)).map(|w| (k, w))).collect();
        if due.is_empty() {
            return;
        }
        self.fit_uniforms.write(g, self.bake.layouts[0], due.len() as u32, due.iter(), |buf, &(_, w)| {
            let f = fit_of(w.2);
            for (k, v) in f.into_iter().enumerate() {
                put_f32(buf, k * 4, v);
            }
        });
        for (n, &(k, w)) in due.iter().enumerate() {
            let Some((model, vertex_cnt, index_cnt, skin)) = decor.model_of(g, k as u32, blocks) else { continue };
            let Some((fit_group, offset)) = self.fit_uniforms.at(n as u32) else { break };
            let layer = object();
            g.view(layer, self.atlas, dimension::D2, 0, 1, k as u32, 1);
            let pass = object();
            g.pass(pass, encoder, &pass_desc(layer, load::CLEAR, self.depth_view, load::CLEAR, 1.0, Stamp::UNTIMED));
            g.set_pipeline(pass, self.bake.pipeline);
            g.set_bind_group(pass, 0, fit_group, &[offset]);
            g.set_bind_group(pass, 1, skin, &[]);
            draw_model(g, pass, &self.bake, model, vertex_cnt, index_cnt, 1);
            g.end(pass);
            release(pass);
            release(layer);
            self.fit[k] = fit_of(w.2);
            self.baked[k] = Some(w);
        }
        let words: Vec<u8> = self.fit.iter().flatten().flat_map(|v| v.to_le_bytes()).collect();
        g.write_buffer(self.fits, 0, &words);
    }

    /// Writes every impostor draw's uniform, in one write.
    pub fn prepare(&mut self, g: &Gpu, draws: &[u32], uniforms: &[f32]) {
        let mine = || draws.as_chunks::<DRAW_WORDS>().0.iter().filter(|d| d[4] == PIPELINE_IMPOSTOR);
        self.uniforms.write(g, self.draw.layouts[1], mine().count() as u32, mine(), |buf, d| {
            let u = d[3] as usize * UNIFORM_FLOATS;
            put_f32(buf, 0, uniforms[u]);
            put_f32(buf, 4, uniforms[u + 1]);
            put_f32(buf, 8, uniforms[u + 2]);
            put_u32(buf, 12, uniforms[u + 3] as u32);
            put_u32(buf, 16, d[2]);
            put_f32(buf, 20, uniforms[u + 4]);
            put_u32(buf, 24, d[1]);
            put_u32(buf, 28, 0);
        });
    }

    /// The impostors into the open pass, after `prepare`: a group whose
    /// layer is not drawn yet is not drawn.
    pub fn draw(&mut self, g: &Gpu, pass: u32, draws: &[u32], blocks: &mut Blocks) -> u32 {
        let mut drawn = 0;
        for (k, d) in draws.as_chunks::<DRAW_WORDS>().0.iter().filter(|d| d[4] == PIPELINE_IMPOSTOR).enumerate() {
            let Some((group, offset)) = self.uniforms.at(k as u32) else { break };
            if self.baked.get(d[1] as usize).copied().flatten().is_none() {
                continue;
            }
            let Some(block) = blocks.group(g, d[0], self.draw.layouts[3]) else { continue };
            if drawn == 0 {
                g.set_pipeline(pass, self.draw.pipeline);
                g.set_bind_group(pass, 0, self.frames.frame, &[]);
                g.set_bind_group(pass, 2, self.group, &[]);
            }
            g.set_bind_group(pass, 1, group, &[offset]);
            g.set_bind_group(pass, 3, block, &[]);
            g.draw(pass, 4, d[2], 0, 0);
            drawn += 1;
        }
        drawn
    }

    pub fn release(&mut self) {
        self.uniforms.release();
        self.fit_uniforms.release();
        self.frames.release();
        for id in [self.group, self.fits, self.depth_view, self.depth, self.atlas_view, self.atlas, self.sampler] {
            release(id);
        }
    }
}
