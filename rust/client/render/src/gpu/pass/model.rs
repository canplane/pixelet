// pxl-render/gpu/pass/model.rs

//! What the model passes (decor, impostors, actors) share: group 0, per-draw
//! uniform slots written once before the pass, and the draw loop.

use xpute_gpu::enums::{AddressMode, FilterMode, MipmapFilterMode};
use xpute_guest::global::Global;

use crate::frame::draw::{DRAW_WORDS, MODEL_VERTEX_WORDS, PIPELINE_SKINNED};
use crate::frame::shader::word::DRAW_FLOATS;
use crate::gpu::api::{
    buf_entry, buf_range_entry, buffer_usage, release, sampler_entry, view_entry, word_bytes,
    Entry, Gpu, Image, TrackedRenderPass,
};
use crate::gpu::atmosphere::AIR_ENTRIES;
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::put::{put_f32, put_u32, Scratch};

fn frame_group(
    g: &Gpu,
    layout: u32,
    frame: u32,
    shadow: [Entry; 2],
    air: [Entry; AIR_ENTRIES],
) -> u32 {
    let mut e = vec![buf_entry(0, frame), shadow[0], shadow[1]];
    e.extend(air);
    g.bind_group(layout, &e)
}

/// Each skin image's bind group, made the first time a frame prepares a draw
/// of it, over one sampler: an image's texture is taken once (`Gpu::image`),
/// so a skin one pass binds again, or another binds, is the group here. As
/// many as the skins the asset table names, in the one layout a skin takes
/// (record.rs `SKIN`), kept for the device's life.
static SKINS: Global<Skins> = Global::new();

#[derive(Default)]
struct Skins {
    sampler: u32,
    /// An image and its group.
    groups: Vec<(u32, u32)>,
}

/// A skin's bind group, made in `layout` where none is; none while its image
/// lands. Only as a frame prepares, before its transaction (`frame_begin`): a
/// group made in a frame that is dropped would be gone and still named here.
pub fn skin_group(g: &Gpu, layout: u32, (image, file): Image) -> Option<u32> {
    if let Some(group) = skin_made(image) {
        return Some(group);
    }
    let skins = SKINS.get(Skins::default);
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

/// A skin's bind group as a frame's draws bind it: the one a prepare made
/// (`skin_group`), or none.
pub fn skin_made(image: u32) -> Option<u32> {
    SKINS
        .get(Skins::default)
        .groups
        .iter()
        .find(|&&(i, _)| i == image)
        .map(|&(_, group)| group)
}

/// The device gone, its groups go with it.
pub(crate) fn on_skins_lost() {
    SKINS.set(Skins::default());
}

/// Which of a model's pipelines draws it: lit into the world's image, or its
/// depth from the sun into the shadow map.
#[derive(Clone, Copy)]
pub enum Stage {
    Lit,
    Depth,
}

/// A layout is shared by its words, so one group 0 serves every model pipeline.
pub fn frame_layout(g: &Gpu, pipelines: &mut Pipelines) -> u32 {
    pipelines.build_world(g, PIPELINE_SKINNED).layouts[0]
}

/// Group 0 as the models read it: the frame, the sun's shadow map `shadow`
/// and the air.
pub struct FrameGroups {
    pub frame: u32,
}

impl FrameGroups {
    pub fn new(
        g: &Gpu,
        layout: u32,
        frame_buf: u32,
        shadow: [Entry; 2],
        air: [Entry; AIR_ENTRIES],
    ) -> FrameGroups {
        FrameGroups {
            frame: frame_group(g, layout, frame_buf, shadow, air),
        }
    }

    pub fn release(&self) {
        release(self.frame);
    }
}

pub struct DrawUniforms {
    bytes: u32,
    /// `bytes` rounded up to the device's offset alignment.
    stride: u32,
    buf: u32,
    cap: u32,
    group: u32,
    scratch: Scratch,
}

#[derive(Clone, Copy)]
pub struct Slots {
    group: u32,
    stride: u32,
    first: u32,
}

impl Slots {
    pub fn at(&self, k: u32) -> (u32, u32) {
        (self.group, (self.first + k) * self.stride)
    }

    /// The slots past the first `n`, as `Iterator::skip` takes them.
    pub fn skip(self, n: u32) -> Slots {
        Slots {
            first: self.first + n,
            ..self
        }
    }
}

impl DrawUniforms {
    pub fn new(bytes: u32, align: u32) -> DrawUniforms {
        DrawUniforms {
            bytes,
            stride: bytes.next_multiple_of(align),
            buf: 0,
            cap: 0,
            group: 0,
            scratch: Scratch::default(),
        }
    }

    /// One write: a later write to the same buffer before the submit would stand
    /// for every draw. Grows to the next power of two, eight at least. Written
    /// only where the lists the draws are made from `moved`, or it grew: else
    /// the buffer holds these bytes already.
    pub fn write<T>(
        &mut self,
        g: &Gpu,
        layout: u32,
        cnt: u32,
        draws: impl Iterator<Item = T>,
        moved: bool,
        mut put: impl FnMut(&mut [u8], T),
    ) {
        if cnt == 0 {
            return;
        }
        let need = self.stride * cnt.next_power_of_two().max(8);
        if !moved && need <= self.cap {
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
        let buf = self.scratch.take(cnt * self.stride);
        for (k, draw) in draws.take(cnt as usize).enumerate() {
            put(&mut buf[k * stride..(k + 1) * stride], draw);
        }
        g.write_buf(self.buf, 0, buf);
    }

    pub fn slots(&self) -> Option<Slots> {
        (self.group != 0).then_some(Slots {
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

/// A draw's uniform, a slot of `DrawUniforms` each: what every model pass
/// writes and its shader reads (frame/shader/word.rs `DRAW_FLOATS`).
pub const DRAW_BYTES: u32 = word_bytes(DRAW_FLOATS);

/// A decor draw's uniform as instance.wgsl's `Draw` reads it, for the
/// model tier and the impostors both: translation, first lane word,
/// stride, opacity, `third`, index list's first word. `third` is decor's
/// padding or the impostor's atlas layer.
pub fn put_instances(buf: &mut [u8], u: &[f32], third: u32) {
    put_f32(buf, 0, u[0]);
    put_f32(buf, 4, u[1]);
    put_f32(buf, 8, u[2]);
    put_u32(buf, 12, u[3] as u32);
    put_u32(buf, 16, u[5] as u32);
    put_f32(buf, 20, u[4]);
    put_u32(buf, 24, third);
    put_u32(buf, 28, u[6] as u32);
}

pub struct ModelDraw {
    pub groups: [u32; 2],
    /// None for a strip of four corners an instance.
    pub mesh: Option<(u32, u32, u32)>,
    pub instances: u32,
}

/// Each draw `resolve` makes, at its slot's uniforms; the pipeline and
/// group 0 set once a draw is made.
pub fn draw_models<'a>(
    pass: &mut TrackedRenderPass,
    lit: &Built,
    pipeline: u32,
    frame: u32,
    slots: Slots,
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
    lit: &Built,
    mesh: u32,
    vertex_cnt: u32,
    idx_cnt: u32,
    instances: u32,
) {
    bind_mesh(pass, lit, mesh, vertex_cnt, idx_cnt);
    pass.draw_indexed(idx_cnt, instances, 0, 0);
}

/// A mesh's vertex lanes and indices, a lane its `lit` stride's share.
pub fn bind_mesh(
    pass: &mut TrackedRenderPass,
    lit: &Built,
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
