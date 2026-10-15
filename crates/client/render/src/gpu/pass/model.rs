// pxl-render/gpu/pass/model.rs

//! What the model passes (decor, impostors, actors) share: group 0, per-draw
//! uniform slots written once before the pass, and the draw loop.

use crate::frame::draw::{DRAW_WORDS, MODEL_VERTEX_WORDS, PIPELINE_SKINNED};
use crate::gpu::api::{buffer_usage, object, release, Gpu};
use crate::gpu::api_spec::{format, resource};
use crate::gpu::atmosphere::AIR_WORDS;
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::renderer::{frame_group, put_f32, put_u32, Scratch};

#[derive(Clone, Copy)]
pub enum Stage {
    Lit,
    Prepass,
}

/// A layout is shared by its words, so one group 0 serves every model pipeline.
pub fn frame_layout(g: &Gpu, pipelines: &mut Pipelines) -> u32 {
    pipelines.build(g, PIPELINE_SKINNED, format::CANVAS).layouts[0]
}

pub struct FrameGroups {
    pub frame: u32,
    layout: u32,
    air: [u32; AIR_WORDS],
}

impl FrameGroups {
    pub fn new(g: &Gpu, layout: u32, frame_buffer: u32, ao: [u32; 5], air: [u32; AIR_WORDS]) -> FrameGroups {
        FrameGroups {
            frame: frame_group(g, layout, frame_buffer, ao, air),
            layout,
            air,
        }
    }

    pub fn rebind(&mut self, g: &Gpu, frame_buffer: u32, ao: [u32; 5]) {
        self.release();
        *self = FrameGroups::new(g, self.layout, frame_buffer, ao, self.air);
    }

    pub fn release(&self) {
        release(self.frame);
    }
}

pub struct DrawUniforms {
    /// Rounded up to the device's offset alignment.
    bytes: u32,
    stride: u32,
    buffer: u32,
    capacity: u32,
    group: u32,
    scratch: Scratch,
}

#[derive(Clone, Copy)]
pub struct Slots {
    group: u32,
    stride: u32,
}

impl Slots {
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

    /// One write: a later write to the same buffer before the submit would stand
    /// for every draw. Grows to the next power of two, eight at least.
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

/// `third` is decor's padding or the impostor's sprite layer.
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

/// A group or a mesh the previous draw bound is not bound again.
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
