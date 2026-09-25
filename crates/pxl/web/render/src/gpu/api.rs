// pxl-render/gpu/api.rs

//! The WebGPU calls the renderer makes, as records in one range of the
//! kernel's memory: the stream (api.json's `stream`). The kernel writes a
//! turn's calls into it, and the host runs them once the turn has returned
//! (apps/web/src/kernel/render/gpu.ts), so nothing but that range and the turn
//! connects the two. Every object is named by a handle the kernel hands out
//! and the host keeps at that handle's slot. Which object, made from what,
//! drawn when, is the renderer's; the host translates each record and makes
//! the call.
//!
//! A record carries everything it names, bytes included. Nothing it reads
//! lies outside it, so what the kernel does to its memory after writing one
//! — a bake reusing its range, a scratch written for the next pass — cannot
//! reach what the host runs. The price is one copy of what is uploaded, and
//! a range that bounds what a turn may upload: an upload asks for `room`
//! first and waits a turn when there is none, which every uploader already
//! does for a device that is not there yet.
//!
//! A pass's description, a bind group's entries and a sampler's filters are
//! words (api.json's codes).

use xpute_runtime::abi::handle::HandleTable;

use crate::gpu::api_spec::{op, stream};

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

/// The stream's words, and how many of them past HEAD the turn has written.
struct Stream {
    words: &'static mut [u32],
    len: usize,
}

const WORDS: usize = (stream::BYTES / 4) as usize;
const HEAD: usize = stream::HEAD as usize;

static STREAM: Global<Option<Stream>> = Global::new();

/// The renderer's calls, recorded into the stream. One and stateless: what
/// it writes to is the stream the module installed.
pub struct Gpu(());

static GPU: Gpu = Gpu(());

/// Installs the stream at `at`, `stream::BYTES` of the kernel's memory that
/// nothing else writes.
///
/// # Safety
/// `at` must address that range, 8-aligned, for as long as the module runs.
pub unsafe fn set_gpu_stream(at: u32) {
    // SAFETY: the caller's range.
    let words = unsafe { core::slice::from_raw_parts_mut(at as usize as *mut u32, WORDS) };
    STREAM.set(Some(Stream { words, len: 0 }));
}

/// The calls; none under a native test, which has no stream.
pub fn gpu() -> Option<&'static Gpu> {
    STREAM.get(|| None).as_ref().map(|_| &GPU)
}

fn stream() -> &'static mut Stream {
    STREAM.get(|| None).as_mut().expect("a GPU call with no stream")
}

/// Ends the turn's records: the host runs what END says once the turn has
/// returned, and the next turn writes from the start.
pub fn gpu_publish() {
    if let Some(s) = STREAM.get(|| None).as_mut() {
        s.words[stream::END as usize] = s.len as u32;
        s.len = 0;
    }
}

/// Whether an upload of `bytes` may go in this turn: taken, it leaves the
/// stream `stream::RESERVE` for what cannot wait.
pub fn room(bytes: u32) -> bool {
    STREAM.get(|| None).as_ref().is_some_and(|s| {
        let left = (WORDS - HEAD - s.len) * 4;
        // A record's head and its arguments, at most eight words.
        left >= bytes as usize + 32 + stream::RESERVE as usize
    })
}

impl Stream {
    fn reserve(&mut self, words: usize) -> usize {
        let at = HEAD + self.len;
        assert!(at + words <= WORDS, "a turn's GPU records past the stream's {} bytes", stream::BYTES);
        self.len += words;
        at
    }

    /// A record of `op`: its arguments, then any words, then any bytes.
    fn record(&mut self, code: u32, args: &[u32], tail: &[u32], bytes: Option<&[u8]>) {
        let byte_words = bytes.map_or(0, |b| 1 + b.len().div_ceil(4));
        let n = args.len() + tail.len() + byte_words;
        let at = self.reserve(2 + n);
        let w = &mut self.words[at..at + 2 + n];
        w[0] = code;
        w[1] = n as u32;
        w[2..2 + args.len()].copy_from_slice(args);
        w[2 + args.len()..2 + args.len() + tail.len()].copy_from_slice(tail);
        if let Some(b) = bytes {
            let k = 2 + args.len() + tail.len();
            w[k] = b.len() as u32;
            let dst = &mut w[k + 1..];
            if let Some(last) = dst.last_mut() {
                *last = 0;
            }
            // SAFETY: the words reserved, as bytes; at least `b.len()` of them.
            unsafe { core::ptr::copy_nonoverlapping(b.as_ptr(), dst.as_mut_ptr() as *mut u8, b.len()) };
        }
    }
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
        let s = stream();
        let at = s.reserve(2 + 4 + record.len() + layouts.len());
        let w = &mut s.words[at..];
        w[..6].copy_from_slice(&[op::PIPELINE, (4 + record.len() + layouts.len()) as u32, id, module, format, record.len() as u32]);
        w[6..6 + record.len()].copy_from_slice(record);
        w[6 + record.len()..6 + record.len() + layouts.len()].copy_from_slice(layouts);
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

    /// A render pass: colour view, colour load, clear r g b a
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
        assert!(at + 2 <= HEAD, "a GPU span past the stream's head");
        let w = &stream().words;
        f64::from_bits(w[at] as u64 | (w[at + 1] as u64) << 32)
    }

    /// Runs a 2D paint stream (api.json's `paint`) over the host's scratch.
    pub fn paint(&self, ops: &[u32]) {
        stream().record(op::PAINT, &[], ops, None);
    }

    /// Copies the scratch's corner into a texture's layer at (x, y).
    pub fn paint_copy(&self, texture: u32, x: u32, y: u32, layer: u32, width: u32, height: u32) {
        put(op::PAINT_COPY, &[texture, x, y, layer, width, height]);
    }
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
    unsafe { core::slice::from_raw_parts(lane.as_ptr() as *const u8, core::mem::size_of_val(lane)) }
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

/// A render pass's description: colour, cleared to transparent or loaded; depth cleared to `depth_clear`, loaded or read only;
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
