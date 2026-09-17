// pxl-render/gpu/pass/skinned.rs

//! Actors on the device: the scheduler's skinned draws (crates/pxl/kernel/src/io/load.rs)
//! executed against each model's mesh block and each actor's joint palette,
//! both as the scheduler wrote them, under shader/skinned.wgsl. A model's skin
//! is the image its asset loaded, by model id.
//!
//! The world's pass lights them under the sun's shadow; a case of the variant
//! picker draws its one actor through a pass of its own, at one sample, with a
//! blank map where the shadow would be.

use crate::frame::draw::{DRAW_WORDS, MODEL_VERTEX_WORDS, PIPELINE_SKINNED, UNIFORM_FLOATS};
use crate::frame::shader::record::PIPELINE_SKINNED_DEPTH;
use crate::gpu::api::{buffer_usage, object, release, GpuApi};
use crate::gpu::api_spec::{address, compare, filter, format, resource};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::renderer::{frame_group, put_f32, skin_group, Blocks, Scratch};

/// shader/skinned.wgsl's Draw: translation, the turn about Z, the opacity, the
/// scale, padding.
const DRAW_UNIFORM_BYTES: u32 = 32;
const SKINS: usize = 16;

#[derive(Clone, Copy, Default)]
struct Skin {
    model: u32,
    image: u32,
    /// Texture, view, group.
    ids: [u32; 3],
    /// When the slot was last asked for, so the one longest unasked is the one
    /// a new model takes.
    seq: u32,
}

pub struct SkinnedPass {
    lit: Built,
    depth: Built,
    frame_group: u32,
    light_group: u32,
    sampler: u32,
    stride: u32,
    draw_buffer: u32,
    draw_bytes: u32,
    draw_group: u32,
    scratch: Scratch,
    skins: [Skin; SKINS],
    /// Counts `set_skin` calls, to stamp the slot each one lands in.
    seq: u32,
}

impl SkinnedPass {
    /// Over `frame_buffer` and a shadow map's entries; `light` the light's frame
    /// and its entries where the pass draws into the shadow map too.
    pub fn new(g: &GpuApi, pipelines: &mut Pipelines, samples: u32, frame_buffer: u32, shadow: [u32; 10], light: Option<(u32, [u32; 10])>, align: u32) -> SkinnedPass {
        let lit = pipelines.build(g, PIPELINE_SKINNED, format::CANVAS, samples);
        let sampler = object();
        (g.sampler)(sampler, filter::LINEAR, filter::LINEAR, filter::NEAREST, address::REPEAT, compare::NONE, 1);
        SkinnedPass {
            depth: pipelines.build(g, PIPELINE_SKINNED_DEPTH, format::CANVAS, samples),
            frame_group: frame_group(g, lit.layouts[0], frame_buffer, shadow),
            light_group: light.map_or(0, |(frame, entries)| frame_group(g, lit.layouts[0], frame, entries)),
            lit,
            sampler,
            stride: DRAW_UNIFORM_BYTES.max(align),
            draw_buffer: 0,
            draw_bytes: 0,
            draw_group: 0,
            scratch: Scratch::default(),
            skins: [Skin::default(); SKINS],
            seq: 0,
        }
    }

    pub fn set_samples(&mut self, g: &GpuApi, pipelines: &mut Pipelines, samples: u32) {
        self.lit = pipelines.build(g, PIPELINE_SKINNED, format::CANVAS, samples);
        self.depth = pipelines.build(g, PIPELINE_SKINNED_DEPTH, format::CANVAS, samples);
    }

    /// Model `model`'s skin; the same image again changes nothing, and one not
    /// yet arrived is asked for again next frame.
    pub fn set_skin(&mut self, g: &GpuApi, model: u32, image: u32) {
        let at = self
            .skins
            .iter()
            .position(|s| s.model == model && s.ids[2] != 0)
            .or_else(|| self.skins.iter().position(|s| s.ids[2] == 0))
            // Nothing tells the pass a model is gone, so a slot is never freed
            // on its own: with the table full a new model would go untextured
            // for good. The character preview walks every shipped variant,
            // more of them than there are slots. The slot asked for longest
            // ago is the one no longer drawn.
            .unwrap_or_else(|| self.skins.iter().enumerate().min_by_key(|(_, s)| s.seq).map_or(0, |(i, _)| i));
        self.seq += 1;
        let seq = self.seq;
        let s = &mut self.skins[at];
        if s.model == model && s.image == image && s.ids[2] != 0 {
            s.seq = seq;
            return;
        }
        for id in s.ids.into_iter().rev() {
            release(id);
        }
        *s = Skin::default();
        if let Some(ids) = skin_group(g, self.lit.layouts[2], self.sampler, image) {
            *s = Skin { model, image, ids, seq };
        }
    }

    fn skin_of(&self, model: u32) -> Option<u32> {
        self.skins.iter().find(|s| s.model == model && s.ids[2] != 0).map(|s| s.ids[2])
    }

    /// Writes every skinned draw's uniforms in one write.
    pub fn prepare(&mut self, g: &GpuApi, draws: &[u32], uniforms: &[f32]) {
        let n = draws.chunks_exact(DRAW_WORDS).filter(|d| d[4] == PIPELINE_SKINNED).count() as u32;
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
        for (slot, d) in draws.chunks_exact(DRAW_WORDS).filter(|d| d[4] == PIPELINE_SKINNED).enumerate() {
            let (at, u) = (slot * stride, d[3] as usize * UNIFORM_FLOATS);
            for i in 0..8 {
                put_f32(buf, at + i * 4, if i < 6 { uniforms[u + i] } else { 0.0 });
            }
        }
        (g.write_buffer)(self.draw_buffer, 0, buf);
    }

    pub fn draw(&mut self, g: &GpuApi, pass: u32, draws: &[u32], blocks: &mut Blocks) -> u32 {
        self.draw_pipeline(g, pass, draws, blocks, self.lit.pipeline, self.frame_group)
    }

    /// The actors' depth from the light; nothing for a pass with no light.
    pub fn draw_depth(&mut self, g: &GpuApi, pass: u32, draws: &[u32], blocks: &mut Blocks) -> u32 {
        if self.light_group == 0 {
            return 0;
        }
        self.draw_pipeline(g, pass, draws, blocks, self.depth.pipeline, self.light_group)
    }

    fn draw_pipeline(&mut self, g: &GpuApi, pass: u32, draws: &[u32], blocks: &mut Blocks, pipeline: u32, group0: u32) -> u32 {
        let mut slot = 0;
        let mut drawn = 0;
        for d in draws.chunks_exact(DRAW_WORDS) {
            if d[4] != PIPELINE_SKINNED {
                continue;
            }
            let offset = slot * self.stride;
            slot += 1;
            let (Some((mesh, vertex_cnt, index_cnt)), Some(skin)) = (blocks.get(d[0]), self.skin_of(d[2])) else {
                continue;
            };
            let Some(palette) = blocks.group(g, d[1], self.lit.layouts[3]) else { continue };
            if self.draw_group == 0 {
                continue;
            }
            if drawn == 0 {
                (g.set_pipeline)(pass, pipeline);
                (g.set_bind_group)(pass, 0, group0, &[]);
            }
            (g.set_bind_group)(pass, 1, self.draw_group, &[offset]);
            (g.set_bind_group)(pass, 2, skin, &[]);
            (g.set_bind_group)(pass, 3, palette, &[]);
            let mut at = 0;
            for lane in 0..self.lit.lanes {
                let size = vertex_cnt * self.lit.strides[lane];
                (g.set_vertex_buffer)(pass, lane as u32, mesh, at, size);
                at += size;
            }
            (g.set_index_buffer)(pass, mesh, vertex_cnt * MODEL_VERTEX_WORDS * 4, index_cnt * 4);
            (g.draw_indexed)(pass, index_cnt, 1, 0, 0);
            drawn += 1;
        }
        drawn
    }

    /// Lets go of what the pass made on the device.
    pub fn release(self) {
        for s in self.skins {
            for id in s.ids.into_iter().rev() {
                release(id);
            }
        }
        for id in [self.draw_group, self.draw_buffer, self.frame_group, self.light_group, self.sampler] {
            release(id);
        }
    }
}
