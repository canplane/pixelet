// pxl_render/gpu/api.rs

//! The WebGPU calls the renderer makes, as the host installs them: one table
//! of functions over numbers, every object named by a handle the kernel hands
//! out and the host keeps at that handle's slot. Which object, made from what,
//! drawn when, is the renderer's; the host translates each call and makes it.
//! On the web the table is the module's imports (pxl-wasm,
//! apps/web/src/wasm/gpu.ts), since a WebAssembly module reaches WebGPU no
//! other way; a native host would put wgpu behind the same table.
//!
//! A pass's description, a bind group's entries and a sampler's filters are
//! words (api.json's codes); a string is its bytes where they lie.

use xpute_core::ds::handle::HandleTable;

use xpute_core::sys::global::Global;

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

pub struct GpuApi {
    /// Every object the host holds let go: the renderer starts over.
    pub reset: fn(),
    pub buffer: fn(id: u32, size: u32, usage: u32),
    pub texture: fn(id: u32, width: u32, height: u32, layers: u32, format: u32, mips: u32, samples: u32, usage: u32),
    /// A texture of an image a load holds, at the image's size, with the
    /// image in it; false while the image has not arrived.
    pub image_texture: fn(id: u32, image: u32) -> bool,
    pub view: fn(id: u32, texture: u32, dimension: u32, base_mip: u32, mips: u32, base_layer: u32, layers: u32),
    pub sampler: fn(id: u32, mag: u32, min: u32, mipmap: u32, address: u32, compare: u32, anisotropy: u32),
    pub shader: fn(id: u32, code: &str),
    /// A bind group layout: (binding, stages, kind) words, laid out at
    /// `samples` a pixel where a kind depends on it.
    pub layout: fn(id: u32, entries: &[u32], samples: u32),
    /// A pipeline from its record (shader.rs), its group layouts, its
    /// shader module, the format it draws into and its sample count.
    pub pipeline: fn(id: u32, record: &[u32], layouts: &[u32], module: u32, format: u32, samples: u32),
    /// A bind group: (binding, resource kind, object, offset, size — 0 the
    /// whole buffer) words.
    pub bind_group: fn(id: u32, layout: u32, entries: &[u32]),
    pub destroy: fn(id: u32),
    pub write_buffer: fn(id: u32, offset: u32, bytes: &[u8]),
    /// A texture's first mip of one layer, whole.
    pub write_texture: fn(id: u32, layer: u32, width: u32, height: u32, bytes: &[u8]),
    pub encoder: fn(id: u32),
    /// A render pass: colour view, resolve view, colour load, clear r g b a
    /// (f32 bits), depth view, depth load, depth clear (f32 bits), query set,
    /// the begin and end timestamp indices; NONE or 0 for what is absent.
    pub pass: fn(id: u32, encoder: u32, desc: &[u32]),
    pub set_pipeline: fn(pass: u32, pipeline: u32),
    pub set_bind_group: fn(pass: u32, index: u32, group: u32, offsets: &[u32]),
    /// A size of 0 is the rest of the buffer.
    pub set_vertex_buffer: fn(pass: u32, slot: u32, buffer: u32, offset: u32, size: u32),
    pub set_index_buffer: fn(pass: u32, buffer: u32, offset: u32, size: u32),
    pub draw: fn(pass: u32, vertices: u32, instances: u32, first_vertex: u32, first_instance: u32),
    pub draw_indexed: fn(pass: u32, indices: u32, instances: u32, first_index: u32, first_instance: u32),
    pub end: fn(pass: u32),
    pub submit: fn(encoder: u32),
    /// A surface's context configured on the device, in the canvas's format.
    pub configure: fn(surface: u32),
    pub resize: fn(surface: u32, width: u32, height: u32),
    /// A view of the surface's texture for this frame.
    pub current: fn(surface: u32, view: u32),
    pub query_set: fn(id: u32, count: u32),
    /// Reads the two timestamps an encoder's passes wrote, once the GPU has them.
    pub timestamps: fn(encoder: u32, query_set: u32),
    /// The GPU's time over the last frame read, in milliseconds.
    pub gpu_ms: fn() -> f64,
    /// Runs a 2D paint stream (api.json's `paint`) over the host's scratch.
    pub paint: fn(ops: &[u32]),
    /// Copies the scratch's corner into a texture's layer at (x, y).
    pub paint_copy: fn(texture: u32, x: u32, y: u32, layer: u32, width: u32, height: u32),
}

static API: Global<Option<GpuApi>> = Global::new();

/// Installs the host's calls.
pub fn set_gpu_api(api: GpuApi) {
    API.set(Some(api));
}

/// The host's calls; none under a native test.
pub fn gpu() -> Option<&'static GpuApi> {
    API.get(|| None).as_ref()
}

/// Objects the host may hold at once: every block on the GPU, and the groups
/// and views made over them.
pub const OBJECTS: usize = 1 << 15;

static OBJECT_TABLE: Global<HandleTable<OBJECTS>> = Global::new();

/// A handle for an object about to be made.
pub fn object() -> u32 {
    OBJECT_TABLE.get(HandleTable::new).acquire().expect("the GPU's object table is full")
}

/// Lets an object go, on the host and its handle with it; 0 is nothing.
pub fn release(id: u32) {
    if id != 0 && OBJECT_TABLE.get(HandleTable::new).release(id) {
        if let Some(g) = gpu() {
            (g.destroy)(id);
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
    unsafe { core::slice::from_raw_parts(lane.as_ptr() as *const u8, core::mem::size_of_val(lane)) }
}

/// A group's entry over a buffer, whole.
pub fn buffer_entry(binding: u32, buffer: u32) -> [u32; 5] {
    [binding, super::api_spec::resource::BUFFER, buffer, 0, 0]
}

/// A render pass's description: colour (and its resolve), cleared to
/// transparent or loaded; depth cleared to `depth_clear`, loaded or read only;
/// and the timestamps written at its begin and end, where a query set is given.
#[allow(clippy::too_many_arguments)]
pub fn pass_desc(color: u32, resolve: u32, color_load: u32, depth: u32, depth_load: u32, depth_clear: f32, query_set: u32, begin: u32, end: u32) -> [u32; 13] {
    [color, resolve, color_load, 0, 0, 0, 0, depth, depth_load, depth_clear.to_bits(), query_set, begin, end]
}
