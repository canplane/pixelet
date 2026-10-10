// pxl-render/gpu/api/record.rs

//! The renderer's calls as records in the kernel's memory, which the page runs
//! after the turn. A record carries everything it names, bytes included, so
//! the kernel reusing memory afterwards cannot reach what the page runs.

use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;
use xpute_runtime::global::Global;
use xpute_runtime::ipc::stream::{Stream, WORD};
use xpute_runtime::mem::base::ptr_at;

use crate::gpu::api_spec::{op, stream};

const HEAD: usize = stream::HEAD as usize;

static STREAM: Global<Option<Stream>> = Global::new();

pub struct Gpu(());

static GPU: Gpu = Gpu(());

/// # Safety
/// `at` must address `stream::BYTES` that nothing else writes, 8-aligned, for
/// as long as the module runs.
pub unsafe fn install(at: u32) -> u32 {
    // SAFETY: the caller's range.
    let words = unsafe { core::slice::from_raw_parts_mut(ptr_at::<u32>(at as usize), stream::BYTES as usize / WORD) };
    STREAM.set(Some(Stream::new(words, HEAD)));
    at
}

pub fn gpu() -> Option<&'static Gpu> {
    STREAM.get(|| None).as_ref().map(|_| &GPU)
}

fn stream() -> &'static mut Stream {
    STREAM.get(|| None).as_mut().or_bug(Errno::ENODEV)
}

pub fn gpu_publish() {
    if let Some(s) = STREAM.get(|| None).as_mut() {
        s.publish();
    }
}

/// A frame's records are one: past the range they are dropped whole.
pub fn frame_begin() {
    if let Some(s) = STREAM.get(|| None).as_mut() {
        s.begin();
    }
}

/// False where the frame did not fit and nothing of it was recorded.
pub fn frame_commit() -> bool {
    STREAM.get(|| None).as_mut().is_none_or(|s| s.commit())
}

/// The browser paces a page's frames to what the GPU finishes.
pub fn frame_room() -> bool {
    true
}

// Each upload's arguments, which its record takes as an array of this length.
const WRITE_BUFFER_ARGS: usize = 2;
const WRITE_TEXTURE_ARGS: usize = 4;

/// Whether an upload of `bytes` fits, leaving `stream::RESERVE` for what cannot wait.
pub fn room(bytes: u32) -> bool {
    let args = if WRITE_BUFFER_ARGS > WRITE_TEXTURE_ARGS { WRITE_BUFFER_ARGS } else { WRITE_TEXTURE_ARGS };
    STREAM.get(|| None).as_ref().is_some_and(|s| s.fits(args, Some(bytes as usize), stream::RESERVE as usize))
}

fn put(code: u32, args: &[u32]) {
    stream().record(code, args, &[], None);
}

impl Gpu {
    pub fn buffer(&self, id: u32, size: u32, usage: u32) {
        put(op::BUFFER, &[id, size, usage]);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn texture(&self, id: u32, width: u32, height: u32, layers: u32, format: u32, mips: u32, usage: u32) {
        put(op::TEXTURE, &[id, width, height, layers, format, mips, usage]);
    }

    /// The host has the image before the kernel hears of it, so one the kernel names is there.
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

    pub fn layout(&self, id: u32, entries: &[u32]) {
        stream().record(op::LAYOUT, &[id], entries, None);
    }

    pub fn pipeline(&self, id: u32, record: &[u32], layouts: &[u32], module: u32, format: u32) {
        let w = stream().open(op::PIPELINE, 4 + record.len() + layouts.len());
        w[..4].copy_from_slice(&[id, module, format, record.len() as u32]);
        w[4..4 + record.len()].copy_from_slice(record);
        w[4 + record.len()..].copy_from_slice(layouts);
    }

    /// Entries are (binding, resource kind, object, offset, size; 0 the whole buffer).
    pub fn bind_group(&self, id: u32, layout: u32, entries: &[u32]) {
        stream().record(op::BIND_GROUP, &[id, layout], entries, None);
    }

    pub fn destroy(&self, id: u32) {
        put(op::DESTROY, &[id]);
    }

    pub fn write_buffer(&self, id: u32, offset: u32, bytes: &[u8]) {
        let args: [u32; WRITE_BUFFER_ARGS] = [id, offset];
        stream().record(op::WRITE_BUFFER, &args, &[], Some(bytes));
    }

    pub fn write_texture(&self, id: u32, layer: u32, width: u32, height: u32, bytes: &[u8]) {
        let args: [u32; WRITE_TEXTURE_ARGS] = [id, layer, width, height];
        stream().record(op::WRITE_TEXTURE, &args, &[], Some(bytes));
    }

    pub fn encoder(&self, id: u32) {
        put(op::ENCODER, &[id]);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn copy_texture(&self, encoder: u32, source: u32, x: u32, y: u32, target: u32, tx: u32, ty: u32, layer: u32, width: u32, height: u32) {
        put(op::COPY_TEXTURE, &[encoder, source, x, y, target, tx, ty, layer, width, height]);
    }

    /// NONE or 0 for what is absent.
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

    pub fn configure(&self, surface: u32) {
        put(op::CONFIGURE, &[surface]);
    }

    pub fn resize(&self, surface: u32, width: u32, height: u32) {
        put(op::RESIZE, &[surface, width, height]);
    }

    pub fn current(&self, surface: u32, view: u32) {
        put(op::CURRENT, &[surface, view]);
    }

    pub fn query_set(&self, id: u32, count: u32) {
        put(op::QUERY_SET, &[id, count]);
    }

    pub fn timestamps(&self, encoder: u32, query_set: u32, written: u32) {
        put(op::TIMESTAMPS, &[encoder, query_set, written]);
    }

    /// Span `span`'s time in ms from the last frame read; past the last span, the whole frame.
    pub fn gpu_ms(&self, span: u32) -> f64 {
        let at = stream::GPU_MS as usize + 2 * span as usize;
        xpute_core::ensure!(at + 2 <= HEAD, ENOSPC);
        let w = stream().head();
        f64::from_bits(w[at] as u64 | (w[at + 1] as u64) << 32)
    }
}
