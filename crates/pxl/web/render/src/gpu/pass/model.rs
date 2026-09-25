// pxl-render/gpu/pass/model.rs

//! What the passes that draw a model a draw share on the device: group 0
//! over the frame and over the light, a dynamic uniform slot a draw written
//! in one write before the pass, and a model's mesh block bound as its
//! vertex lanes and its indices. Which draws are a pass's own, what its
//! uniform says and what it binds beside them — a joint palette, an instance
//! block, a skin — stay the pass's (skinned.rs, decor.rs).

use crate::frame::draw::MODEL_VERTEX_WORDS;
use crate::gpu::api::{buffer_usage, object, release, Gpu};
use crate::gpu::api_spec::resource;
use crate::gpu::atmosphere::AIR_WORDS;
use crate::gpu::pipeline::Built;
use crate::gpu::renderer::{frame_group, Scratch};

/// Group 0 as the eye draws with it and as the light does; the light's is
/// none for a pass that never draws into a shadow map.
pub struct FrameGroups {
    pub frame: u32,
    pub light: u32,
    /// The air's entries, which the groups are made with again on a rebind.
    air: [u32; AIR_WORDS],
}

impl FrameGroups {
    /// Over `frame_buffer` and a shadow map's entries, and over the light's
    /// frame and its entries where there is one; `ao` the occlusion map's entry.
    pub fn new(g: &Gpu, layout: u32, frame_buffer: u32, shadow: [u32; 10], light: Option<(u32, [u32; 10])>, ao: [u32; 5], air: [u32; AIR_WORDS]) -> FrameGroups {
        FrameGroups {
            frame: frame_group(g, layout, frame_buffer, shadow, ao, air),
            light: light.map_or(0, |(frame, entries)| frame_group(g, layout, frame, entries, ao, air)),
            air,
        }
    }

    /// Both again over a new occlusion map's entry `ao`.
    pub fn rebind(&mut self, g: &Gpu, layout: u32, frame_buffer: u32, shadow: [u32; 10], light: Option<(u32, [u32; 10])>, ao: [u32; 5]) {
        self.release();
        *self = FrameGroups::new(g, layout, frame_buffer, shadow, light, ao, self.air);
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

    /// The group and slot `k`'s offset in it; none before the first write.
    pub fn at(&self, k: u32) -> Option<(u32, u32)> {
        (self.group != 0).then_some((self.group, k * self.stride))
    }

    pub fn release(&mut self) {
        release(self.group);
        release(self.buffer);
        self.scratch.free();
    }
}

/// Binds `mesh`, a model block of `vertex_cnt` vertices in `lit`'s lanes and
/// then `index_cnt` indices, and draws `instances` of it.
pub fn draw_model(g: &Gpu, pass: u32, lit: &Built, mesh: u32, vertex_cnt: u32, index_cnt: u32, instances: u32) {
    let mut at = 0;
    for lane in 0..lit.lanes {
        let size = vertex_cnt * lit.strides[lane];
        g.set_vertex_buffer(pass, lane as u32, mesh, at, size);
        at += size;
    }
    g.set_index_buffer(pass, mesh, vertex_cnt * MODEL_VERTEX_WORDS * 4, index_cnt * 4);
    g.draw_indexed(pass, index_cnt, instances, 0, 0);
}
