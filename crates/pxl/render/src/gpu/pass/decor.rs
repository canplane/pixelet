// pxl-render/gpu/pass/decor.rs

//! Decor on the device: the scheduler's decor draws (draw.rs) executed
//! as instanced draws of each variant's model, the instances read straight out
//! of the decor block the scheduler built (crates/pxl/kernel/src/layer/decor/build.rs's layout) as it lies
//! on the GPU: the block is the instance buffer, the chunk's translation is its
//! uniform, and the scheduler says which block a chunk shows.
//!
//! A model is the scheduler's (crates/pxl/kernel/src/io/load.rs), registered by the decor asset
//! store and drawn from the same block the kernel holds its triangles in for
//! the hit test — the vertex lanes of crates/pxl/kernel/src/io/model_asset.rs's layout, and the indices
//! past them. The groups are every bundle's variants in bundle order, the order
//! the kinds were taught; a group's skin is the image its variant loaded, held
//! by image rather than by group, since two variants — two kinds, even — may
//! name the same skin and a texture is uploaded per row, not per group.

use crate::frame::draw::{DRAW_WORDS, MODEL_VERTEX_WORDS, PIPELINE_DECOR, PIPELINE_GHOST_DECOR, UNIFORM_FLOATS};
use crate::frame::shader::record::PIPELINE_DECOR_DEPTH;
use crate::gpu::api::{buffer_usage, object, release, GpuApi};
use crate::gpu::api_spec::{address, compare, filter, format, resource};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::renderer::{frame_group, put_f32, put_u32, skin_group, Blocks, Scratch};
use crate::gpu::shadow::Shadow;

/// A decor draw's dynamic uniform (shader/decor.wgsl's Draw): translation, the
/// group's first lane word, the instance count, the opacity, padding.
const DRAW_UNIFORM_BYTES: u32 = 32;
/// Decor groups the pass holds a model and a skin for: every bundle's
/// variants, in bundle order.
pub const GROUPS: usize = 64;

/// No skin row taken yet.
const NO_SKIN: usize = usize::MAX;

#[derive(Clone, Copy)]
struct Group {
    /// The model's mesh block, by the draw list's handle, and its skin's image.
    mesh: u32,
    image: Option<u32>,
    /// The skin row this group draws with.
    skin: usize,
}

impl Default for Group {
    fn default() -> Group {
        Group { mesh: 0, image: None, skin: NO_SKIN }
    }
}

/// One image on the device, shared by every group whose variant names it.
#[derive(Clone, Copy, Default)]
struct Skin {
    image: u32,
    /// Texture, view, group.
    ids: [u32; 3],
    /// Groups drawing with this row; the row goes back when the last leaves.
    uses: u32,
}

fn is_decor(pipeline: u32) -> bool {
    pipeline == PIPELINE_DECOR || pipeline == PIPELINE_GHOST_DECOR
}

pub struct DecorPass {
    lit: Built,
    ghost: Built,
    depth: Built,
    frame_group: u32,
    light_group: u32,
    sampler: u32,
    stride: u32,
    draw_buffer: u32,
    draw_bytes: u32,
    draw_group: u32,
    scratch: Scratch,
    groups: [Group; GROUPS],
    skins: [Skin; GROUPS],
}

impl DecorPass {
    pub fn new(g: &GpuApi, pipelines: &mut Pipelines, samples: u32, frame_buffer: u32, shadow: &Shadow, align: u32) -> DecorPass {
        let lit = pipelines.build(g, PIPELINE_DECOR, format::CANVAS, samples);
        let sampler = object();
        (g.sampler)(sampler, filter::LINEAR, filter::LINEAR, filter::NEAREST, address::REPEAT, compare::NONE, 1);
        DecorPass {
            ghost: pipelines.build(g, PIPELINE_GHOST_DECOR, format::CANVAS, samples),
            depth: pipelines.build(g, PIPELINE_DECOR_DEPTH, format::CANVAS, samples),
            frame_group: frame_group(g, lit.layouts[0], frame_buffer, shadow.entries(1)),
            light_group: frame_group(g, lit.layouts[0], shadow.light_frame, shadow.blank_entries(1)),
            lit,
            sampler,
            stride: DRAW_UNIFORM_BYTES.max(align),
            draw_buffer: 0,
            draw_bytes: 0,
            draw_group: 0,
            scratch: Scratch::default(),
            groups: [Group::default(); GROUPS],
            skins: [Skin::default(); GROUPS],
        }
    }

    pub fn set_samples(&mut self, g: &GpuApi, pipelines: &mut Pipelines, samples: u32) {
        self.lit = pipelines.build(g, PIPELINE_DECOR, format::CANVAS, samples);
        self.ghost = pipelines.build(g, PIPELINE_GHOST_DECOR, format::CANVAS, samples);
        self.depth = pipelines.build(g, PIPELINE_DECOR_DEPTH, format::CANVAS, samples);
    }

    /// Each group's model block and skin image as the kernel has them now, in
    /// group order (0 for a model not yet registered); a group whose model or
    /// skin changed lets its skin go.
    pub fn set_models(&mut self, groups: &[(u32, Option<u32>)]) {
        for (group, &(mesh, image)) in groups.iter().take(GROUPS).enumerate() {
            let gr = self.groups[group];
            if gr.mesh != mesh || gr.image != image {
                self.groups[group] = Group { mesh, image, skin: NO_SKIN };
                self.release_skin(gr.skin);
            }
        }
    }

    /// A group's model block and counts and its skin's group, the skin made on
    /// first use once its image is here.
    fn model_of(&mut self, g: &GpuApi, group: u32, blocks: &Blocks) -> Option<(u32, u32, u32, u32)> {
        let gr = *self.groups.get(group as usize)?;
        let (buffer, vertex_cnt, index_cnt) = blocks.get(gr.mesh)?;
        let at = if gr.skin == NO_SKIN { self.take_skin(g, gr.image?)? } else { gr.skin };
        self.groups[group as usize].skin = at;
        Some((buffer, vertex_cnt, index_cnt, self.skins[at].ids[2]))
    }

    /// The row `image` is on, made on first use and shared from then on: two
    /// variants naming one skin are one texture on the device.
    fn take_skin(&mut self, g: &GpuApi, image: u32) -> Option<usize> {
        if let Some(at) = self.skins.iter().position(|s| s.uses > 0 && s.image == image) {
            self.skins[at].uses += 1;
            return Some(at);
        }
        let at = self.skins.iter().position(|s| s.uses == 0)?;
        let ids = skin_group(g, self.lit.layouts[2], self.sampler, image)?;
        self.skins[at] = Skin { image, ids, uses: 1 };
        Some(at)
    }

    /// One group less on a row; its texture, view and group go back once the
    /// last group has left it.
    fn release_skin(&mut self, at: usize) {
        let Some(s) = self.skins.get_mut(at) else { return };
        s.uses = s.uses.saturating_sub(1);
        if s.uses == 0 {
            for id in s.ids.into_iter().rev() {
                release(id);
            }
            *s = Skin::default();
        }
    }

    /// Triangles one instance of `group` draws; 0 while it loads.
    pub fn triangles_of(&self, group: u32, blocks: &Blocks) -> u32 {
        self.groups.get(group as usize).and_then(|gr| blocks.get(gr.mesh)).map_or(0, |(_, _, index_cnt)| index_cnt / 3)
    }

    /// Writes every decor draw's uniforms, ghosts included, in one write: a
    /// later write to the same buffer before the submit would stand for every draw.
    pub fn prepare(&mut self, g: &GpuApi, draws: &[u32], uniforms: &[f32]) {
        let n = draws.chunks_exact(DRAW_WORDS).filter(|d| is_decor(d[4])).count() as u32;
        if n == 0 {
            return;
        }
        let need = self.stride * n.next_power_of_two().max(8);
        if need > self.draw_bytes {
            release(self.draw_group);
            release(self.draw_buffer);
            self.draw_buffer = object();
            (g.buffer)(self.draw_buffer, need, buffer_usage::UNIFORM | buffer_usage::COPY_DST);
            self.draw_group = object();
            (g.bind_group)(self.draw_group, self.lit.layouts[1], &[0, resource::BUFFER, self.draw_buffer, 0, DRAW_UNIFORM_BYTES]);
            self.draw_bytes = need;
        }
        let stride = self.stride as usize;
        let buf = self.scratch.take(n * self.stride);
        for (slot, d) in draws.chunks_exact(DRAW_WORDS).filter(|d| is_decor(d[4])).enumerate() {
            let (at, u) = (slot * stride, d[3] as usize * UNIFORM_FLOATS);
            put_f32(buf, at, uniforms[u]);
            put_f32(buf, at + 4, uniforms[u + 1]);
            put_f32(buf, at + 8, uniforms[u + 2]);
            put_u32(buf, at + 12, uniforms[u + 3] as u32);
            put_u32(buf, at + 16, d[2]);
            put_f32(buf, at + 20, uniforms[u + 4]);
            put_u32(buf, at + 24, 0);
            put_u32(buf, at + 28, 0);
        }
        (g.write_buffer)(self.draw_buffer, 0, buf);
    }

    /// The opaque decor into the open pass, after `prepare`.
    pub fn draw(&mut self, g: &GpuApi, pass: u32, draws: &[u32], blocks: &mut Blocks) -> u32 {
        self.draw_pipeline(g, pass, draws, blocks, PIPELINE_DECOR, self.lit.pipeline, self.frame_group)
    }

    /// The opaque decor's depth from the light.
    pub fn draw_depth(&mut self, g: &GpuApi, pass: u32, draws: &[u32], blocks: &mut Blocks) -> u32 {
        self.draw_pipeline(g, pass, draws, blocks, PIPELINE_DECOR, self.depth.pipeline, self.light_group)
    }

    /// The ghost's decor, blended, after everything opaque.
    pub fn draw_ghosts(&mut self, g: &GpuApi, pass: u32, draws: &[u32], blocks: &mut Blocks) -> u32 {
        self.draw_pipeline(g, pass, draws, blocks, PIPELINE_GHOST_DECOR, self.ghost.pipeline, self.frame_group)
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_pipeline(&mut self, g: &GpuApi, pass: u32, draws: &[u32], blocks: &mut Blocks, which: u32, pipeline: u32, group0: u32) -> u32 {
        let mut slot = 0;
        let mut drawn = 0;
        for d in draws.chunks_exact(DRAW_WORDS) {
            if !is_decor(d[4]) {
                continue;
            }
            let offset = slot * self.stride;
            slot += 1;
            if d[4] != which || self.draw_group == 0 {
                continue;
            }
            let Some((model, vertex_cnt, index_cnt, skin)) = self.model_of(g, d[1], blocks) else { continue };
            let Some(block) = blocks.group(g, d[0], self.lit.layouts[3]) else { continue };
            if drawn == 0 {
                (g.set_pipeline)(pass, pipeline);
                (g.set_bind_group)(pass, 0, group0, &[]);
            }
            (g.set_bind_group)(pass, 1, self.draw_group, &[offset]);
            (g.set_bind_group)(pass, 2, skin, &[]);
            (g.set_bind_group)(pass, 3, block, &[]);
            let mut at = 0;
            for lane in 0..self.lit.lanes {
                let size = vertex_cnt * self.lit.strides[lane];
                (g.set_vertex_buffer)(pass, lane as u32, model, at, size);
                at += size;
            }
            (g.set_index_buffer)(pass, model, vertex_cnt * MODEL_VERTEX_WORDS * 4, index_cnt * 4);
            (g.draw_indexed)(pass, index_cnt, d[2], 0, 0);
            drawn += 1;
        }
        drawn
    }
}
