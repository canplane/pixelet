// pxl-render/gpu/pass/model.rs

//! What the model passes share: the decor, the impostors and the skinned
//! characters. They bind one group 0, which holds the frame words, the light's
//! shadow map and the air. Each pass writes its draws' uniforms once before
//! the pass, one slot a draw, and draws them by one loop.

use xpute_core::global::Global;
use xpute_gpu::enums::{AddressMode, FilterMode, MipmapFilterMode};
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::frame::shader::record::PIPELINE_SKINNED;
use crate::frame::shader::word::{instance, DRAW_FLOATS};
use crate::gpu::api::{
    buf_entry, buf_range_entry, buffer_usage, bytes, release, sampler_entry, view_entry,
    word_bytes, Entry, Gpu, Image, Layout, TrackedRenderPass,
};
use crate::gpu::atmosphere::AIR_ENTRIES;
use crate::gpu::draw::{Block, DRAW_WORDS, MODEL_VERTEX_WORDS};
use crate::gpu::pipeline::{Pipeline, PipelineCache};
use crate::gpu::put::put_u32;

fn frame_group(
    g: &Gpu,
    layout: Layout,
    frame: u32,
    shadow: [Entry; 2],
    air: [Entry; AIR_ENTRIES],
) -> u32 {
    let mut e = vec![buf_entry(0, frame), shadow[0], shadow[1]];
    e.extend(air);
    g.bind_group(layout, &e)
}

/// The bind group of each skin image, all over one sampler. A group is made
/// the first time a frame prepares a draw of its image. `Gpu::image` hands
/// an image's texture over only once, so every later bind of that skin, by
/// any pass, uses the group kept here. There are as many groups as skins the
/// asset table names, all in the one skin layout (record.rs `SKIN`), and
/// they are kept for the device's life.
static SKINS: Global<SkinCache> = Global::new();

#[derive(Default)]
struct SkinCache {
    sampler: u32,
    /// Pairs of an image and its bind group.
    groups: Vec<(u32, u32)>,
}

/// The bind group of a skin image, made in `layout` if it has none yet. It is
/// none while the image is still landing. Call it only while a frame
/// prepares, before the frame's transaction begins (`frame_begin`). A group
/// made inside a frame that is then dropped would be gone, and this table
/// would still name it.
pub fn skin_group(g: &Gpu, layout: Layout, (image, file): Image) -> Option<u32> {
    if let Some(group) = skin_made(image) {
        return Some(group);
    }
    let skins = SKINS.get(SkinCache::default);
    let texture = g.image(image, file)?;
    if skins.sampler == 0 {
        skins.sampler = g.sampler(
            FilterMode::linear,
            FilterMode::linear,
            MipmapFilterMode::nearest,
            AddressMode::repeat,
            None,
            1,
        );
    }
    let view = g.view(texture, false, 0, 1, 0, 1);
    let group = g.bind_group(
        layout,
        &[view_entry(0, view), sampler_entry(1, skins.sampler)],
    );
    skins.groups.push((image, group));
    Some(group)
}

/// The bind group a prepare made for skin `image` (`skin_group`), for a
/// frame's draws to bind. It makes none.
pub fn skin_made(image: u32) -> Option<u32> {
    SKINS
        .get(SkinCache::default)
        .groups
        .iter()
        .find(|&&(i, _)| i == image)
        .map(|&(_, group)| group)
}

/// Forgets every skin group once the device is lost, because the groups
/// were the device's.
pub(crate) fn on_skins_lost() {
    SKINS.set(SkinCache::default());
}

/// Which of a model's two pipelines draws it. `Lit` draws it lit into the
/// world's color. `Depth` draws its depth, seen from the light, into the shadow
/// map.
#[derive(Clone, Copy)]
pub enum Stage {
    Lit,
    Depth,
}

/// The group 0 layout of every model pipeline. Pipelines whose groups read
/// alike share one layout (gpu/pipeline.rs), so a group made in the skinned
/// pipeline's layout binds to every model pipeline.
pub fn frame_layout(g: &Gpu, pipelines: &mut PipelineCache) -> Layout {
    pipelines.build_world(g, PIPELINE_SKINNED).layouts[0]
}

/// The model passes' group 0. It binds the frame words, the light's shadow map
/// and the air.
pub struct FrameGroup {
    pub frame: u32,
}

impl FrameGroup {
    pub fn new(
        g: &Gpu,
        layout: Layout,
        frame_buf: u32,
        shadow: [Entry; 2],
        air: [Entry; AIR_ENTRIES],
    ) -> FrameGroup {
        FrameGroup {
            frame: frame_group(g, layout, frame_buf, shadow, air),
        }
    }

    pub fn release(&self) {
        release(self.frame);
    }
}

pub struct UniformBuffer {
    bytes: u32,
    /// The distance between slots: `bytes` rounded up to the device's
    /// dynamic offset alignment.
    stride: u32,
    buf: u32,
    cap: u32,
    group: u32,
    scratch: Block,
}

#[derive(Clone, Copy)]
pub struct SlotRange {
    group: u32,
    stride: u32,
    first: u32,
}

impl SlotRange {
    pub fn at(&self, k: u32) -> (u32, u32) {
        (self.group, (self.first + k) * self.stride)
    }

    /// Drops the first `n` slots, as `Iterator::skip` drops the first `n`
    /// draws, so that the slots and the draws stay in step.
    pub fn skip(self, n: u32) -> SlotRange {
        SlotRange {
            first: self.first + n,
            ..self
        }
    }
}

impl UniformBuffer {
    pub fn new(bytes: u32, align: u32) -> UniformBuffer {
        UniformBuffer {
            bytes,
            stride: bytes.next_multiple_of(align),
            buf: 0,
            cap: 0,
            group: 0,
            scratch: Block::default(),
        }
    }

    /// Writes the uniforms of the first `cnt` draws, one slot each, in a
    /// single write. Every queued write lands before the submit runs, so a
    /// second write to the buffer would be what every draw reads. The buffer
    /// grows to the next power of two of slots, eight at least. It is written
    /// only when the lists the draws come from `changed` or when it grew;
    /// otherwise it already holds these bytes.
    pub fn write<T>(
        &mut self,
        g: &Gpu,
        layout: Layout,
        cnt: u32,
        draws: impl Iterator<Item = T>,
        changed: bool,
        mut put: impl FnMut(&mut [u8], T),
    ) {
        if cnt == 0 {
            return;
        }
        let need = self.stride * cnt.next_power_of_two().max(8);
        if !changed && need <= self.cap {
            return;
        }
        if need > self.cap {
            release(self.group);
            release(self.buf);
            self.buf = g.buf(need, buffer_usage::UNIFORM | buffer_usage::COPY_DST);
            self.group = g.bind_group(layout, &[buf_range_entry(0, self.buf, 0, self.bytes)]);
            self.cap = need;
        }
        let stride = self.stride as usize;
        let buf = self.scratch.take(cnt * self.stride).or_bug(Errno::ENOMEM);
        for (k, draw) in draws.take(cnt as usize).enumerate() {
            put(&mut buf[k * stride..(k + 1) * stride], draw);
        }
        g.write_buf(self.buf, 0, buf);
    }

    pub fn slots(&self) -> Option<SlotRange> {
        (self.group != 0).then_some(SlotRange {
            group: self.group,
            stride: self.stride,
            first: 0,
        })
    }

    pub fn release(&mut self) {
        release(self.group);
        release(self.buf);
        self.scratch.free();
    }
}

/// The bytes of one draw's uniform: the part of a `UniformBuffer` slot that
/// every model pass writes and its shader reads (frame/shader/word.rs
/// `DRAW_FLOATS`).
pub const DRAW_BYTES: u32 = word_bytes(DRAW_FLOATS);

/// A model's extent along glTF's axes: across (X), up (Y) and deep (Z).
pub type Extent = [f32; 3];

/// Puts a decor draw's uniform into `buf` as instance.wgsl reads it, for the
/// model tier and the impostors alike. It is the list's words `u`, with
/// `layer` written over the `LAYER` word. For an impostor `layer` is its atlas
/// layer; the model tier passes 0, and its shader reads no layer.
pub fn put_instances(buf: &mut [u8], u: &[f32], layer: u32) {
    buf[..DRAW_BYTES as usize].copy_from_slice(bytes(u));
    put_u32(buf, 4 * instance::LAYER, layer);
}

pub struct ModelDraw {
    pub groups: [u32; 2],
    /// The mesh with its vertex and index counts. None draws a strip of four
    /// corners for each instance.
    pub mesh: Option<(u32, u32, u32)>,
    pub instances: u32,
}

/// Draws each draw that `resolve` makes a `ModelDraw` of. Draw `k` of
/// `draws` reads the uniform in slot `k`, whether or not it is made. The
/// pipeline and group 0 are set only once some draw is made.
pub fn draw_models<'a>(
    pass: &mut TrackedRenderPass,
    lit: &Pipeline,
    pipeline: u32,
    frame: u32,
    slots: SlotRange,
    draws: impl Iterator<Item = &'a [u32; DRAW_WORDS]>,
    mut resolve: impl FnMut(&[u32; DRAW_WORDS]) -> Option<ModelDraw>,
) {
    for (k, d) in draws.enumerate() {
        let Some(m) = resolve(d) else { continue };
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, frame, &[]);
        let (group, off) = slots.at(k as u32);
        pass.set_bind_group(1, group, &[off]);
        for (i, id) in m.groups.into_iter().enumerate() {
            pass.set_bind_group(2 + i as u32, id, &[]);
        }
        match m.mesh {
            Some((mesh, vertex_cnt, idx_cnt)) => {
                draw_model(pass, lit, mesh, vertex_cnt, idx_cnt, m.instances);
            }
            None => pass.draw(4, m.instances, 0, 0),
        }
    }
}

pub fn draw_model(
    pass: &mut TrackedRenderPass,
    lit: &Pipeline,
    mesh: u32,
    vertex_cnt: u32,
    idx_cnt: u32,
    instances: u32,
) {
    bind_mesh(pass, lit, mesh, vertex_cnt, idx_cnt);
    pass.draw_indexed(idx_cnt, instances, 0, 0);
}

/// Binds a mesh buffer's vertex lanes and its indices. The buffer holds the
/// lanes back to back, each `vertex_cnt` vertices at the `lit` pipeline's
/// stride for that lane, and the u32 indices after them all.
pub fn bind_mesh(
    pass: &mut TrackedRenderPass,
    lit: &Pipeline,
    mesh: u32,
    vertex_cnt: u32,
    idx_cnt: u32,
) {
    let mut at = 0;
    for lane in 0..lit.lanes {
        let size = vertex_cnt * lit.strides[lane];
        pass.set_vertex_buf(lane as u32, mesh, at, size);
        at += size;
    }
    pass.set_idx_buf(mesh, vertex_cnt * MODEL_VERTEX_WORDS * 4, idx_cnt * 4);
}
