// pxl-render/gpu/api/record.rs

//! The renderer's calls as records in the kernel's memory, which the page runs
//! after the turn. A record carries numbers and the address and length of
//! the bytes it uploads, read when the page replays the turn: the range
//! lives unwritten until then (`op::UPLOAD_BUFFER`). An owner whose range
//! does is named as it is; any other's bytes are copied into the stream,
//! which lives until then, and the copy is named.

use xpute_guest::global::Global;
use xpute_guest::ipc::stream::{Stream, WORD};
use xpute_guest::mem::base::ptr_at;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::gpu::api_spec::{op, stream};

const HEAD: usize = stream::HEAD as usize;

static STREAM: Global<Option<Stream>> = Global::new();

/// Where the stream's words begin in the module's memory.
static BASE: Global<u32> = Global::new();

pub struct Gpu(());

static GPU: Gpu = Gpu(());

/// # Safety
/// `at` must address `stream::BYTES` that nothing else writes, 8-aligned, for
/// as long as the module runs.
pub unsafe fn install(at: u32) -> u32 {
    // SAFETY: the caller's range.
    let words = unsafe { core::slice::from_raw_parts_mut(ptr_at::<u32>(at as usize), stream::BYTES as usize / WORD) };
    STREAM.set(Some(Stream::new(words, HEAD)));
    BASE.set(at);
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

// Each upload's arguments: its own, then the bytes' address and length.
const UPLOAD_BUFFER_ARGS: usize = 4;
const UPLOAD_TEXTURE_ARGS: usize = 6;

/// Whether a copy of `bytes` into the stream fits, leaving `stream::RESERVE`
/// for what cannot wait.
pub fn room(bytes: u32) -> bool {
    STREAM
        .get(|| None)
        .as_ref()
        .is_some_and(|s| s.fits(UPLOAD_TEXTURE_ARGS, Some(bytes as usize), stream::RESERVE as usize))
}

/// An upload whose bytes are copied into the stream: `args` ends in the
/// address and length, the address made that of the copy once it is in.
fn upload_copied(code: u32, args: &mut [u32], bytes: &[u8]) {
    let s = stream();
    let words = s.left() / WORD;
    let n = args.len();
    s.record(code, args, &[], Some(bytes));
    // A dropped record took no room; one taken began where the room did.
    if s.left() / WORD == words {
        return;
    }
    let total = stream::BYTES as usize / WORD;
    let record = *BASE.get(|| 0) as usize + (total - words) * WORD;
    // The record: its code and length, its arguments, the bytes' length, the bytes.
    args[n - 2] = (record + (2 + n + 1) * WORD) as u32;
    // SAFETY: the stream's own words, just recorded, the address argument among them.
    unsafe { *ptr_at::<u32>(record + (2 + n - 2) * WORD) = args[n - 2] };
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

    /// The host has the image before the kernel hears of it, so one the kernel
    /// names is there; the texture takes it, so each image is uploaded once.
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

    /// The module's one compute entry, over `layouts`.
    pub fn compute_pipeline(&self, id: u32, module: u32, layouts: &[u32]) {
        stream().record(op::COMPUTE_PIPELINE, &[id, module], layouts, None);
    }

    /// Entries are (binding, resource kind, object, offset, size; 0 the whole buffer).
    pub fn bind_group(&self, id: u32, layout: u32, entries: &[u32]) {
        stream().record(op::BIND_GROUP, &[id, layout], entries, None);
    }

    pub fn destroy(&self, id: u32) {
        put(op::DESTROY, &[id]);
    }

    /// `bytes` copied into the stream: any range, however soon its owner
    /// rewrites it.
    pub fn write_buffer(&self, id: u32, offset: u32, bytes: &[u8]) {
        let mut args: [u32; UPLOAD_BUFFER_ARGS] = [id, offset, 0, bytes.len() as u32];
        upload_copied(op::UPLOAD_BUFFER, &mut args, bytes);
    }

    /// `bytes` read where they are: their owner keeps them unwritten until
    /// the page replays the turn.
    pub fn upload_buffer(&self, id: u32, offset: u32, bytes: &[u8]) {
        put(op::UPLOAD_BUFFER, &[id, offset, bytes.as_ptr() as u32, bytes.len() as u32]);
    }

    pub fn write_texture(&self, id: u32, layer: u32, width: u32, height: u32, bytes: &[u8]) {
        let mut args: [u32; UPLOAD_TEXTURE_ARGS] = [id, layer, width, height, 0, bytes.len() as u32];
        upload_copied(op::UPLOAD_TEXTURE, &mut args, bytes);
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

    pub fn compute_pass(&self, id: u32, encoder: u32) {
        put(op::COMPUTE_PASS, &[id, encoder]);
    }

    pub fn dispatch(&self, pass: u32, x: u32, y: u32, z: u32) {
        put(op::DISPATCH, &[pass, x, y, z]);
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

    /// `length` bytes of buffer `id` from `offset`, once the GPU has them,
    /// into the module's memory at `address`, the word at `word` holding
    /// `ticket` then marked done (`op::READ_BUFFER`): a turn or more later,
    /// and only while that word still holds the ticket.
    #[allow(clippy::too_many_arguments)]
    pub fn read_buffer(&self, encoder: u32, id: u32, offset: u32, length: u32, address: u32, word: u32, ticket: u32) {
        put(op::READ_BUFFER, &[encoder, id, offset, length, address, word, ticket]);
    }

    /// Span `span`'s time in ms from the last frame read; past the last span, the whole frame.
    pub fn gpu_ms(&self, span: u32) -> f64 {
        let at = stream::GPU_MS as usize + 2 * span as usize;
        xpute_kit::ensure!(at + 2 <= HEAD, ENOSPC);
        let w = stream().head();
        f64::from_bits(w[at] as u64 | (w[at + 1] as u64) << 32)
    }
}
