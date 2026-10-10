// pxl-render/gpu/api.rs

//! The WebGPU calls the renderer makes. On wasm32 they are records the page
//! runs after the turn (api/record.rs); elsewhere wgpu makes them directly
//! (api/device.rs). Both take the same words, so nothing above knows which.

#[cfg(not(target_arch = "wasm32"))]
mod device;
#[cfg(target_arch = "wasm32")]
mod record;

#[cfg(not(target_arch = "wasm32"))]
pub use device::{frame_begin, frame_commit, frame_room, gpu, gpu_publish, install, room, Gpu};
#[cfg(not(target_arch = "wasm32"))]
pub use device::{hold_image, open_device, surface_bind, surface_unbind, BindRefused, DeviceInfo};
#[cfg(target_arch = "wasm32")]
pub use record::{frame_begin, frame_commit, frame_room, gpu, gpu_publish, install, room, Gpu};

use xpute_runtime::abi::handle::HandleTable;

use crate::gpu::api_spec::resource;

use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;
use xpute_runtime::global::Global;

pub mod buffer_usage {
    pub const COPY_SRC: u32 = 0x0004;
    pub const COPY_DST: u32 = 0x0008;
    pub const INDEX: u32 = 0x0010;
    pub const VERTEX: u32 = 0x0020;
    pub const UNIFORM: u32 = 0x0040;
    pub const STORAGE: u32 = 0x0080;
}

pub mod texture_usage {
    pub const COPY_SRC: u32 = 0x01;
    pub const COPY_DST: u32 = 0x02;
    pub const TEXTURE_BINDING: u32 = 0x04;
    pub const RENDER_ATTACHMENT: u32 = 0x10;
}

pub use crate::gpu::api_spec::stamp::NONE;

pub const OBJECTS: usize = 1 << 15;

static OBJECT_TABLE: Global<HandleTable<OBJECTS>> = Global::new();

pub fn object() -> u32 {
    OBJECT_TABLE.get(HandleTable::new).acquire().or_bug(Errno::ENOSPC)
}

/// 0 is nothing.
pub fn release(id: u32) {
    if id != 0 && OBJECT_TABLE.get(HandleTable::new).release(id) {
        if let Some(g) = gpu() {
            g.destroy(id);
        }
    }
}

pub fn forget_objects() {
    OBJECT_TABLE.set(HandleTable::new());
}

pub fn bytes<T: Copy>(lane: &[T]) -> &[u8] {
    // SAFETY: plain numbers read as their bytes, for as long as the lane is borrowed.
    unsafe { core::slice::from_raw_parts(lane.as_ptr().cast::<u8>(), core::mem::size_of_val(lane)) }
}

pub const fn word_bytes(words: usize) -> u32 {
    (words * 4) as u32
}

pub fn buffer_entry(binding: u32, buffer: u32) -> [u32; 5] {
    [binding, resource::BUFFER, buffer, 0, 0]
}

/// Color cleared to transparent or loaded; depth cleared to `depth_clear`, loaded or read only.
pub fn pass_desc(color: u32, color_load: u32, depth: u32, depth_load: u32, depth_clear: f32, stamp: [u32; 3]) -> [u32; 12] {
    [color, color_load, 0, 0, 0, 0, depth, depth_load, depth_clear.to_bits(), stamp[0], stamp[1], stamp[2]]
}

/// A span's pair of timestamp queries: the first pass writes the begin, every
/// pass the end, so the last one's stands. A span with no passes reads 0.
#[derive(Clone, Copy)]
pub struct Stamp {
    set: u32,
    begin: u32,
    end: u32,
}

impl Stamp {
    pub const NONE: Stamp = Stamp { set: 0, begin: NONE, end: NONE };
    pub const UNTIMED: [u32; 3] = [0, NONE, NONE];

    pub fn of(set: u32, span: usize) -> Stamp {
        if set == 0 {
            return Stamp::NONE;
        }
        let begin = 2 * span as u32;
        Stamp { set, begin, end: begin + 1 }
    }

    pub fn written(&self) -> bool {
        self.set != 0 && self.begin == NONE
    }

    pub fn pass(&mut self) -> [u32; 3] {
        let words = [self.set, self.begin, self.end];
        self.begin = NONE;
        words
    }
}
