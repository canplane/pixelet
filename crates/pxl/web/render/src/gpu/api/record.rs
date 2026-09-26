// pxl-render/gpu/api/record.rs

//! The renderer's calls as records in one range of the kernel's memory: the
//! stream (api.json's `stream`). The kernel writes a turn's calls into it, and
//! the page runs them once the turn has returned
//! (apps/web/src/kernel/render/gpu.ts), so nothing but that range and the turn
//! connects the two. The page keeps each object at its handle's slot.
//!
//! A record carries everything it names, bytes included. Nothing it reads
//! lies outside it, so what the kernel does to its memory after writing one
//! — a bake reusing its range, a scratch written for the next pass — cannot
//! reach what the page runs. The price is one copy of what is uploaded, and
//! a range that bounds what a turn may upload: an upload asks for `room`
//! first and waits a turn when there is none, which every uploader already
//! does for a device that is not there yet.

use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;
use xpute_runtime::global::Global;
use xpute_runtime::ipc::stream::Stream;
use xpute_runtime::mem::base::ptr_at;

use crate::gpu::api_spec::{op, stream};

const HEAD: usize = stream::HEAD as usize;

static STREAM: Global<Option<Stream>> = Global::new();

/// The renderer's calls, recorded into the stream. One and stateless: what
/// it writes to is the stream the module installed.
pub struct Gpu(());

static GPU: Gpu = Gpu(());

/// Installs the stream at `at`, `stream::BYTES` of the kernel's memory that
/// nothing else writes, and names it for the boot block: the page runs what
/// is recorded there.
///
/// # Safety
/// `at` must address that range, 8-aligned, for as long as the module runs.
pub unsafe fn install(at: u32) -> u32 {
    // SAFETY: the caller's range.
    let words = unsafe { core::slice::from_raw_parts_mut(ptr_at::<u32>(at as usize), (stream::BYTES / 4) as usize) };
    STREAM.set(Some(Stream::new(words, HEAD)));
    at
}

/// The calls; none under a native test, which has no stream.
pub fn gpu() -> Option<&'static Gpu> {
    STREAM.get(|| None).as_ref().map(|_| &GPU)
}

fn stream() -> &'static mut Stream {
    STREAM.get(|| None).as_mut().or_bug(Errno::ENODEV)
}

/// Ends the turn's records, for the host to run once the turn has returned.
pub fn gpu_publish() {
    if let Some(s) = STREAM.get(|| None).as_mut() {
        s.publish();
    }
}

/// Whether an upload of `bytes` may go in this turn: taken, it leaves the
/// stream `stream::RESERVE` for what cannot wait.
pub fn room(bytes: u32) -> bool {
    // A record's op, count and arguments, at most eight words.
    STREAM.get(|| None).as_ref().is_some_and(|s| s.left() >= bytes as usize + 32 + stream::RESERVE as usize)
}

fn put(code: u32, args: &[u32]) {
    stream().record(code, args, &[], None);
}

impl Gpu {
    /// Every object the host holds let go: the renderer starts over.
    pub fn reset(&self) {
        put(op::RESET, &[]);
    }

    pub fn buffer(&self, id: u32, size: u32, usage: u32) {
        put(op::BUFFER, &[id, size, usage]);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn texture(&self, id: u32, width: u32, height: u32, layers: u32, format: u32, mips: u32, usage: u32) {
        put(op::TEXTURE, &[id, width, height, layers, format, mips, usage]);
    }

    /// A texture of an image a load holds, at the image's size, with the
    /// image in it. The host has the image before the kernel hears of it
    /// (apps/web/src/kernel/io/load.ts), so one the kernel names is there.
    pub fn image_texture(&self, id: u32, image: u32) {
        put(op::IMAGE_TEXTURE, &[id, image]);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn view(&self, id: u32, texture: u32, dimension: u32, base_mip: u32, mips: u32, base_layer: u32, layers: u32) {
        put(op::VIEW, &[id, texture, dimension, base_mip, mips, base_layer, layers]);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn sampler(&self, id: u32, mag: u32, min: u32, mipmap: u32, address: u32, compare: u32, anisotropy: u32) {
        put(op::SAMPLER, &[id, mag, min, mipmap, address, compare, anisotropy]);
    }

    pub fn shader(&self, id: u32, code: &str) {
        stream().record(op::SHADER, &[id], &[], Some(code.as_bytes()));
    }

    /// A bind group layout: (binding, stages, kind) words.
    pub fn layout(&self, id: u32, entries: &[u32]) {
        stream().record(op::LAYOUT, &[id], entries, None);
    }

    /// A pipeline from its record (shader.rs), its group layouts, its
    /// shader module and the format it draws into.
    pub fn pipeline(&self, id: u32, record: &[u32], layouts: &[u32], module: u32, format: u32) {
        let w = stream().open(op::PIPELINE, 4 + record.len() + layouts.len());
        w[..4].copy_from_slice(&[id, module, format, record.len() as u32]);
        w[4..4 + record.len()].copy_from_slice(record);
        w[4 + record.len()..].copy_from_slice(layouts);
    }

    /// A bind group: (binding, resource kind, object, offset, size — 0 the
    /// whole buffer) words.
    pub fn bind_group(&self, id: u32, layout: u32, entries: &[u32]) {
        stream().record(op::BIND_GROUP, &[id, layout], entries, None);
    }

    pub fn destroy(&self, id: u32) {
        put(op::DESTROY, &[id]);
    }

    pub fn write_buffer(&self, id: u32, offset: u32, bytes: &[u8]) {
        stream().record(op::WRITE_BUFFER, &[id, offset], &[], Some(bytes));
    }

    /// A texture's first mip of one layer, whole.
    pub fn write_texture(&self, id: u32, layer: u32, width: u32, height: u32, bytes: &[u8]) {
        stream().record(op::WRITE_TEXTURE, &[id, layer, width, height], &[], Some(bytes));
    }

    pub fn encoder(&self, id: u32) {
        put(op::ENCODER, &[id]);
    }

    /// A region of `source`'s first mip at (x, y) into `target`'s `layer` at
    /// (tx, ty), recorded into `encoder`.
    #[allow(clippy::too_many_arguments)]
    pub fn copy_texture(&self, encoder: u32, source: u32, x: u32, y: u32, target: u32, tx: u32, ty: u32, layer: u32, width: u32, height: u32) {
        put(op::COPY_TEXTURE, &[encoder, source, x, y, target, tx, ty, layer, width, height]);
    }

    /// A render pass: color view, color load, clear r g b a
    /// (f32 bits), depth view, depth load, depth clear (f32 bits), query set,
    /// the begin and end timestamp indices; NONE or 0 for what is absent.
    pub fn pass(&self, id: u32, encoder: u32, desc: &[u32]) {
        stream().record(op::PASS, &[id, encoder], desc, None);
    }

    pub fn set_pipeline(&self, pass: u32, pipeline: u32) {
        put(op::SET_PIPELINE, &[pass, pipeline]);
    }

    pub fn set_bind_group(&self, pass: u32, index: u32, group: u32, offsets: &[u32]) {
        stream().record(op::SET_BIND_GROUP, &[pass, index, group], offsets, None);
    }

    /// A size of 0 is the rest of the buffer.
    pub fn set_vertex_buffer(&self, pass: u32, slot: u32, buffer: u32, offset: u32, size: u32) {
        put(op::SET_VERTEX_BUFFER, &[pass, slot, buffer, offset, size]);
    }

    pub fn set_index_buffer(&self, pass: u32, buffer: u32, offset: u32, size: u32) {
        put(op::SET_INDEX_BUFFER, &[pass, buffer, offset, size]);
    }

    pub fn draw(&self, pass: u32, vertices: u32, instances: u32, first_vertex: u32, first_instance: u32) {
        put(op::DRAW, &[pass, vertices, instances, first_vertex, first_instance]);
    }

    pub fn draw_indexed(&self, pass: u32, indices: u32, instances: u32, first_index: u32, first_instance: u32) {
        put(op::DRAW_INDEXED, &[pass, indices, instances, first_index, first_instance]);
    }

    pub fn end(&self, pass: u32) {
        put(op::END, &[pass]);
    }

    pub fn submit(&self, encoder: u32) {
        put(op::SUBMIT, &[encoder]);
    }

    /// A surface's context configured on the device, in the canvas's format.
    pub fn configure(&self, surface: u32) {
        put(op::CONFIGURE, &[surface]);
    }

    pub fn resize(&self, surface: u32, width: u32, height: u32) {
        put(op::RESIZE, &[surface, width, height]);
    }

    /// A view of the surface's texture for this frame.
    pub fn current(&self, surface: u32, view: u32) {
        put(op::CURRENT, &[surface, view]);
    }

    pub fn query_set(&self, id: u32, count: u32) {
        put(op::QUERY_SET, &[id, count]);
    }

    /// Reads the timestamps an encoder's passes wrote into the set, once the
    /// GPU has them: the pair of each span set in `written`.
    pub fn timestamps(&self, encoder: u32, query_set: u32, written: u32) {
        put(op::TIMESTAMPS, &[encoder, query_set, written]);
    }

    /// The GPU's time in the last frame the host read, in milliseconds:
    /// span `span`'s (a Stamp's pair), 0 for one no pass wrote; past the last
    /// span, the frame's whole, from the first written begin to the last
    /// written end.
    pub fn gpu_ms(&self, span: u32) -> f64 {
        let at = stream::GPU_MS as usize + 2 * span as usize;
        xpute_core::ensure!(at + 2 <= HEAD, ENOSPC);
        let w = stream().head();
        f64::from_bits(w[at] as u64 | (w[at + 1] as u64) << 32)
    }
}
