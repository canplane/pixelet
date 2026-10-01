// pxl-render/gpu/pass/model.rs

//! What the passes that draw a model a draw share on the device: group 0
//! over the frame and over the light, a dynamic uniform slot a draw written
//! in one write before the pass, the decor block's draw uniform, and the
//! loop that binds a draw's groups and its model block and draws it. Which
//! draws are a pass's own, what its uniform says and what it binds beside
//! them — a joint palette, an instance block, a skin — stay the pass's
//! (skinned.rs, decor.rs, impostor.rs).

use crate::frame::draw::{DRAW_WORDS, MODEL_VERTEX_WORDS, PIPELINE_SKINNED, UNIFORM_FLOATS};
use crate::gpu::api::{buffer_usage, object, release, Gpu};
use crate::gpu::api_spec::{format, resource};
use crate::gpu::atmosphere::AIR_WORDS;
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::renderer::{frame_group, put_f32, put_u32, Scratch};

/// Which of a model pass's pipelines a pass draws with: the lit one, the
/// depth from the light into the shadow map, or the depth from the eye the
/// screen-space occlusion reads.
#[derive(Clone, Copy)]
pub enum Stage {
    Lit,
    Depth,
    Prepass,
}

/// Group 0's layout in every model pipeline — decor's, the impostors' and
/// the actors', lit or depth alone. They declare the one group
/// (frame/shader/record.rs's LIT_FRAME), and a layout is shared by its words
/// (gpu/pipeline.rs), so one group serves all of them.
pub fn frame_layout(g: &Gpu, pipelines: &mut Pipelines) -> u32 {
    pipelines.build(g, PIPELINE_SKINNED, format::CANVAS).layouts[0]
}

/// Group 0 as the eye draws with it and as the light does; the light's is
/// none where nothing is drawn into a shadow map.
pub struct FrameGroups {
    pub frame: u32,
    pub light: u32,
    layout: u32,
    /// The air's entries, which the groups are made with again on a rebind.
    air: [u32; AIR_WORDS],
}

impl FrameGroups {
    /// Over `frame_buffer` and a shadow map's entries, and over the light's
    /// frame and its entries where there is one; `ao` the occlusion map's
    /// entry; in `layout` (`frame_layout`).
    pub fn new(g: &Gpu, layout: u32, frame_buffer: u32, shadow: [u32; 10], light: Option<(u32, [u32; 10])>, ao: [u32; 5], air: [u32; AIR_WORDS]) -> FrameGroups {
        FrameGroups {
            frame: frame_group(g, layout, frame_buffer, shadow, ao, air),
            light: light.map_or(0, |(frame, entries)| frame_group(g, layout, frame, entries, ao, air)),
            layout,
            air,
        }
    }

    /// Both again over a new occlusion map's entry `ao`.
    pub fn rebind(&mut self, g: &Gpu, frame_buffer: u32, shadow: [u32; 10], light: Option<(u32, [u32; 10])>, ao: [u32; 5]) {
        self.release();
        *self = FrameGroups::new(g, self.layout, frame_buffer, shadow, light, ao, self.air);
    }

    pub fn release(&self) {
        release(self.frame);
        release(self.light);
    }
}

/// One uniform slot a draw, `stride` apart, in one buffer the pass binds at
/// each draw's offset.
pub struct DrawUniforms {
    /// What one draw's uniform is, and the slot it takes: the size rounded up
    /// to the device's offset alignment.
    bytes: u32,
    stride: u32,
    buffer: u32,
    capacity: u32,
    group: u32,
    scratch: Scratch,
}

/// Where a pass's draw uniforms are bound: the group, and the offset one
/// slot is from the next.
#[derive(Clone, Copy)]
pub struct Slots {
    group: u32,
    stride: u32,
}

impl Slots {
    /// The group and slot `k`'s offset in it.
    pub fn at(&self, k: u32) -> (u32, u32) {
        (self.group, k * self.stride)
    }
}

impl DrawUniforms {
    pub fn new(bytes: u32, align: u32) -> DrawUniforms {
        DrawUniforms {
            bytes,
            stride: bytes.max(align),
            buffer: 0,
            capacity: 0,
            group: 0,
            scratch: Scratch::default(),
        }
    }

    /// A slot for each of `count` draws, the k-th of `draws` laid out by
    /// `put(slot, draw)`, in one write: a later write to the same buffer
    /// before the submit would stand for every draw. The buffer grows to the
    /// next power of two, eight at least, so a count that wanders does not
    /// make it again every frame.
    pub fn write<T>(&mut self, g: &Gpu, layout: u32, count: u32, draws: impl Iterator<Item = T>, mut put: impl FnMut(&mut [u8], T)) {
        if count == 0 {
            return;
        }
        let need = self.stride * count.next_power_of_two().max(8);
        if need > self.capacity {
            release(self.group);
            release(self.buffer);
            self.buffer = object();
            g.buffer(self.buffer, need, buffer_usage::UNIFORM | buffer_usage::COPY_DST);
            self.group = object();
            g.bind_group(self.group, layout, &[0, resource::BUFFER, self.buffer, 0, self.bytes]);
            self.capacity = need;
        }
        let stride = self.stride as usize;
        let buf = self.scratch.take(count * self.stride);
        for (k, draw) in draws.take(count as usize).enumerate() {
            put(&mut buf[k * stride..(k + 1) * stride], draw);
        }
        g.write_buffer(self.buffer, 0, buf);
    }

    /// Where the slots are bound; none before the first write.
    pub fn slots(&self) -> Option<Slots> {
        (self.group != 0).then_some(Slots {
            group: self.group,
            stride: self.stride,
        })
    }

    pub fn release(&mut self) {
        release(self.group);
        release(self.buffer);
        self.scratch.free();
    }
}

/// A draw's uniform floats, as the draw list holds them.
pub fn uniform_of<'a>(uniforms: &'a [f32], d: &[u32; DRAW_WORDS]) -> &'a [f32] {
    &uniforms[d[3] as usize * UNIFORM_FLOATS..][..UNIFORM_FLOATS]
}

/// A decor block's draw uniform as decor.wgsl's and impostor.wgsl's `Draw`
/// read it, from the draw's floats `u`: the translation, the group's first
/// lane word, its stride, the opacity, `third` — decor's padding, the
/// impostor's sprite layer — and the index list's first word.
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

/// What one model draw binds past its uniform, and how it draws.
pub struct ModelDraw {
    /// Groups 2 and 3: a skin or the sprite sheet, and the block the vertex
    /// shader reads.
    pub groups: [u32; 2],
    /// The model's mesh block and its vertex and index counts; none for a
    /// strip of four corners an instance.
    pub mesh: Option<(u32, u32, u32)>,
    pub instances: u32,
}

/// Every one of `draws` that `resolve` finds on the device, under
/// `pipeline`, whose vertex lanes are `lit`'s; the k-th at uniform slot
/// `first + k`. The pipeline is set at the first draw, and group 0 to
/// `frame` there unless `bound`, the group 0 the pass has, already is it.
/// A group or a mesh the draw before bound is not bound again.
#[allow(clippy::too_many_arguments)]
pub fn draw_models<'a>(
    g: &Gpu,
    pass: u32,
    lit: &Built,
    pipeline: u32,
    (frame, bound): (u32, &mut u32),
    slots: Slots,
    first: u32,
    draws: impl Iterator<Item = &'a [u32; DRAW_WORDS]>,
    mut resolve: impl FnMut(&[u32; DRAW_WORDS]) -> Option<ModelDraw>,
) {
    let mut started = false;
    let mut groups = [0u32; 2];
    let mut mesh_bound = 0;
    for (k, d) in draws.enumerate() {
        let Some(m) = resolve(d) else { continue };
        if !started {
            started = true;
            g.set_pipeline(pass, pipeline);
            if *bound != frame {
                g.set_bind_group(pass, 0, frame, &[]);
                *bound = frame;
            }
        }
        let (group, offset) = slots.at(first + k as u32);
        g.set_bind_group(pass, 1, group, &[offset]);
        for (i, (held, id)) in groups.iter_mut().zip(m.groups).enumerate() {
            if *held != id {
                g.set_bind_group(pass, 2 + i as u32, id, &[]);
                *held = id;
            }
        }
        match m.mesh {
            Some((mesh, vertex_cnt, index_cnt)) => draw_model(g, pass, lit, &mut mesh_bound, mesh, vertex_cnt, index_cnt, m.instances),
            None => g.draw(pass, 4, m.instances, 0, 0),
        }
    }
}

/// Binds `mesh`, a model block of `vertex_cnt` vertices in `lit`'s lanes and
/// then `index_cnt` indices, unless `bound` says it is bound already, and
/// draws `instances` of it.
#[allow(clippy::too_many_arguments)]
pub fn draw_model(g: &Gpu, pass: u32, lit: &Built, bound: &mut u32, mesh: u32, vertex_cnt: u32, index_cnt: u32, instances: u32) {
    if *bound != mesh {
        let mut at = 0;
        for lane in 0..lit.lanes {
            let size = vertex_cnt * lit.strides[lane];
            g.set_vertex_buffer(pass, lane as u32, mesh, at, size);
            at += size;
        }
        g.set_index_buffer(pass, mesh, vertex_cnt * MODEL_VERTEX_WORDS * 4, index_cnt * 4);
        *bound = mesh;
    }
    g.draw_indexed(pass, index_cnt, instances, 0, 0);
}
