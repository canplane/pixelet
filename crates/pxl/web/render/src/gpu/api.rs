// pxl-render/gpu/api.rs

//! The WebGPU calls the renderer makes, `Gpu`'s, and where they go: which
//! object, made from what, drawn when, is the renderer's, and every object is
//! named by a handle the kernel hands out. What makes the call is the target's.
//! A module cannot reach WebGPU itself, so on wasm32 the calls are records the
//! page runs once the turn has returned (api/record.rs); anywhere else the
//! kernel links wgpu and makes each call as it is asked for (api/device.rs).
//! The two answer the same calls with the same words — a pass's description,
//! a bind group's entries and a sampler's filters are api.json's codes — so
//! nothing above this knows which it has.

#[cfg(not(target_arch = "wasm32"))]
mod device;
#[cfg(target_arch = "wasm32")]
mod record;

#[cfg(not(target_arch = "wasm32"))]
pub use device::{gpu, gpu_publish, install, room, Gpu};
#[cfg(not(target_arch = "wasm32"))]
pub use device::{open_device, surface_bind, surface_unbind, BindRefused, DeviceInfo};
#[cfg(target_arch = "wasm32")]
pub use record::{gpu, gpu_publish, install, room, Gpu};

use xpute_runtime::abi::handle::HandleTable;

use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;
use xpute_runtime::global::Global;

/// WebGPU's buffer usage bits.
pub mod buffer_usage {
    pub const MAP_READ: u32 = 0x0001;
    pub const COPY_SRC: u32 = 0x0004;
    pub const COPY_DST: u32 = 0x0008;
    pub const INDEX: u32 = 0x0010;
    pub const VERTEX: u32 = 0x0020;
    pub const UNIFORM: u32 = 0x0040;
    pub const STORAGE: u32 = 0x0080;
    pub const QUERY_RESOLVE: u32 = 0x0200;
}

/// WebGPU's texture usage bits.
pub mod texture_usage {
    pub const COPY_SRC: u32 = 0x01;
    pub const COPY_DST: u32 = 0x02;
    pub const TEXTURE_BINDING: u32 = 0x04;
    pub const RENDER_ATTACHMENT: u32 = 0x10;
}

/// No timestamp written, no attachment: a pass description's empty word.
pub const NONE: u32 = u32::MAX;

/// Objects the host may hold at once: every block on the GPU, and the groups
/// and views made over them.
pub const OBJECTS: usize = 1 << 15;

static OBJECT_TABLE: Global<HandleTable<OBJECTS>> = Global::new();

/// A handle for an object about to be made.
pub fn object() -> u32 {
    OBJECT_TABLE.get(HandleTable::new).acquire().or_bug(Errno::ENOSPC)
}

/// Lets an object go, on the host and its handle with it; 0 is nothing.
pub fn release(id: u32) {
    if id != 0 && OBJECT_TABLE.get(HandleTable::new).release(id) {
        if let Some(g) = gpu() {
            g.destroy(id);
        }
    }
}

/// Forgets every handle: the objects are gone on the host already.
pub fn forget_objects() {
    OBJECT_TABLE.set(HandleTable::new());
}

/// Objects held now.
pub fn object_count() -> u32 {
    OBJECT_TABLE.get(HandleTable::new).count()
}

/// A lane's bytes, where they lie.
pub fn bytes<T: Copy>(lane: &[T]) -> &[u8] {
    // SAFETY: plain numbers read as their bytes, for as long as the lane is borrowed.
    unsafe { core::slice::from_raw_parts(lane.as_ptr().cast::<u8>(), core::mem::size_of_val(lane)) }
}

/// The bytes `words` 32-bit words take — floats or u32s — as a size the
/// device and the heap are told in, from a lane's length as Rust holds it.
pub const fn word_bytes(words: usize) -> u32 {
    (words * 4) as u32
}

/// A group's entry over a buffer, whole.
pub fn buffer_entry(binding: u32, buffer: u32) -> [u32; 5] {
    [binding, crate::gpu::api_spec::resource::BUFFER, buffer, 0, 0]
}

/// A render pass's description: color, cleared to transparent or loaded; depth cleared to `depth_clear`, loaded or read only;
/// and the timestamps the pass writes, as its span's `Stamp::pass` gives them.
pub fn pass_desc(color: u32, color_load: u32, depth: u32, depth_load: u32, depth_clear: f32, stamp: [u32; 3]) -> [u32; 12] {
    [color, color_load, 0, 0, 0, 0, depth, depth_load, depth_clear.to_bits(), stamp[0], stamp[1], stamp[2]]
}

/// Where a span of passes writes its GPU time: a pair of queries in the
/// frame's set, the first pass of the span writing the begin and every pass
/// the end, so the last one's stands. A span is one thing the frame does —
/// the air's tables, the shadow map, the world — however many passes it
/// takes, and the one that took none this frame reads 0.
#[derive(Clone, Copy)]
pub struct Stamp {
    set: u32,
    begin: u32,
    end: u32,
}

impl Stamp {
    /// Writes nothing: no query set, or a pass the frame's time is not about.
    pub const NONE: Stamp = Stamp { set: 0, begin: NONE, end: NONE };
    /// A pass's words where it is timed under no span.
    pub const UNTIMED: [u32; 3] = [0, NONE, NONE];

    /// Span `span`'s pair in `set`; nothing where there is no set.
    pub fn of(set: u32, span: usize) -> Stamp {
        if set == 0 {
            return Stamp::NONE;
        }
        let begin = 2 * span as u32;
        Stamp { set, begin, end: begin + 1 }
    }

    /// Whether a pass of the span wrote its pair.
    pub fn written(&self) -> bool {
        self.set != 0 && self.begin == NONE
    }

    /// The next pass's words: the begin for the first, the end for each.
    pub fn pass(&mut self) -> [u32; 3] {
        let words = [self.set, self.begin, self.end];
        self.begin = NONE;
        words
    }
}
