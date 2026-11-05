// pxl-render/gpu/api.rs

//! The WebGPU calls the renderer makes, written through xpute's encoder
//! (xpute-gpu encode) into one stream in the kernel's memory, which the host
//! runs after the turn: the page's on WebGPU, the device app's on wgpu
//! (xpute-gpu host), the same records either way. What is pixelet's stays
//! here: the pipeline records expanded into descriptors, a pass's shape and
//! the spans' timing.
//!
//! The stream's head holds what the host writes for the kernel: the device's
//! block (limits, features, timestamp period, preferred surface format,
//! status) and the spans' times.

pub use xpute_gpu::encode::OBJECT_OPS;
use xpute_gpu::encode::{self, call, Gpu as Encoder, Id};
use xpute_gpu::enums::{
    AddressMode, BlendFactor, BlendOperation, BufferBindingType, CanvasAlphaMode, CompareFunction,
    CullMode, FilterMode, FrontFace, IndexFormat, LoadOp, MipmapFilterMode, PrimitiveTopology,
    QueryType, SamplerBindingType, StencilOperation, StorageTextureAccess, StoreOp, TextureAspect,
    TextureDimension, TextureFormat, TextureSampleType, TextureViewDimension, VertexFormat,
    VertexStepMode, FEATURE_NAME, LIMIT_NAME,
};
use xpute_gpu::spec::{BUFFER_USAGE, COLOR_WRITE, STATUS, TEXTURE_USAGE};
use xpute_guest::abi::handle::handle_slot;
use xpute_guest::global::Global;
use xpute_guest::ipc::stream::{Stream, WORD};
use xpute_guest::mem;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

/// A fence or the device's STATUS as the host last stored it.
pub use xpute_gpu::encode::load_acquire;
/// Whether a completion's fence holds the ticket it was asked with
/// (xpute spec RESULT).
pub use xpute_gpu::encode::signaled;
pub use xpute_gpu::enums::TextureFormat as Format;
/// What a read back or an image says beside its fence.
pub use xpute_gpu::spec::RESULT;

use crate::api_spec::{record, stream};
use crate::frame::shader::floor::BIND_GROUPS;
use crate::frame::shader::record::{FRAGMENT_ENTRY, VERTEX_ENTRY};
use crate::gpu::pipeline::LANES;
use crate::gpu::span_spec::{GpuSpanName, GPU_SPAN_NAMES};
use crate::gpu::split::Splits;

pub mod buffer_usage {
    pub use xpute_gpu::spec::BUFFER_USAGE::{
        COPY_DST, COPY_SRC, INDEX, INDIRECT, MAP_READ, STORAGE, UNIFORM, VERTEX,
    };
}

pub mod texture_usage {
    pub use xpute_gpu::spec::TEXTURE_USAGE::{
        COPY_DST, COPY_SRC, RENDER_ATTACHMENT, STORAGE_BINDING, TEXTURE_BINDING,
    };
}

/// The objects the renderer may hold at once.
pub const OBJECTS: usize = 1 << 15;

use crate::frame::shader::record::GROUP_ENTRIES;

const SPANS: usize = GPU_SPAN_NAMES;

/// WebGPU's default minUniformBufferOffsetAlignment: an adapter's limit is a
/// power of two no larger, so a larger one is no device's.
pub const UNIFORM_ALIGN_MAX: u32 = 256;

/// An image file as its asker hands it: its name, the asker's own and the
/// same for one file wherever it is handed, and its bytes, handed until its
/// texture lands (`take_landed`) and empty after, while the texture is the
/// taker's (`Gpu::image`).
pub type Image = (u32, &'static [u8]);

/// The device's block (xpute DEVICE): a word a limit, the features a bit
/// each, the timestamp period, the preferred surface format, the frames
/// unfinished, the status.
const FEAT_WORDS: usize = FEATURE_NAME.len().div_ceil(32);
const PERIOD_AT: usize = LIMIT_NAME.len() + FEAT_WORDS;
const DEVICE_WORDS: usize = PERIOD_AT + 4;

/// The head's words past the stream's own: the device's block, then the
/// spans' reads, `READS` of them, each its fence and result, then the
/// begin and end ticks of the frame it timed, two words a tick (`read_at`). The stream says the
/// head's length itself, so the host reads none of this.
const DEVICE_AT: usize = Stream::OWN_WORDS;
const READS_AT: usize = DEVICE_AT + DEVICE_WORDS;
/// The spans' reads on their way at once, each its own mapped buffer: a
/// frame's read lands a frame or two after it, so with three none waits on
/// the one before it to be taken.
pub const READS: usize = 3;
const READ_WORDS: usize = 2 + 2 * 2 * SPANS;
const HEAD: usize = READS_AT + READS * READ_WORDS;

/// Read `k`'s fence in the head; its result and its ticks follow it.
const fn read_at(k: usize) -> usize {
    READS_AT + k * READ_WORDS
}

/// What the device is, once its block is in.
#[derive(Clone, Copy)]
pub struct Device {
    pub align: u32,
    pub timestamps: bool,
    /// What a surface shows best, and so what the canvas's pipelines target.
    pub canvas: Format,
    period_ns: f32,
}

/// A span's pair of timestamp queries: the first pass writes the begin, every
/// pass the end, so the last one's stands. A span with no passes reads 0.
#[derive(Clone, Copy)]
pub struct TimestampWrites {
    set: Id,
    begin: Option<u32>,
    end: u32,
}

impl TimestampWrites {
    pub const NONE: TimestampWrites = TimestampWrites {
        set: 0,
        begin: None,
        end: 0,
    };

    pub fn of(set: Id, span: usize) -> TimestampWrites {
        if set == 0 {
            return TimestampWrites::NONE;
        }
        let begin = 2 * span as u32;
        TimestampWrites {
            set,
            begin: Some(begin),
            end: begin + 1,
        }
    }

    pub fn is_written(&self) -> bool {
        self.set != 0 && self.begin.is_none()
    }

    pub fn pass(&mut self) -> Option<encode::TimestampWrites> {
        let t = (self.set != 0).then_some(encode::TimestampWrites {
            query_set: self.set,
            begin: self.begin,
            end: Some(self.end),
        });
        self.begin = None;
        t
    }
}

/// `texture`'s first mip at `origin` (x, y, layer), every aspect: the place a
/// copy reads or writes.
pub fn place(texture: Id, origin: [u32; 3]) -> encode::TexelCopyTexture {
    encode::TexelCopyTexture {
        texture,
        mip: 0,
        origin,
        aspect: TextureAspect::all,
    }
}

/// An encoder being recorded and the frame's spans: a pass is handed this
/// and writes its own span's.
pub struct RenderContext<'a> {
    pub g: &'a Gpu,
    pub encoder: Id,
    stamps: &'a mut [TimestampWrites; GPU_SPAN_NAMES],
}

impl<'a> RenderContext<'a> {
    pub fn new(g: &'a Gpu, encoder: Id, stamps: &'a mut [TimestampWrites; GPU_SPAN_NAMES]) -> Self {
        Self { g, encoder, stamps }
    }

    pub fn stamp(&mut self, span: GpuSpanName) -> &mut TimestampWrites {
        &mut self.stamps[span as usize]
    }
}

/// What a render pass holds. A group set with offsets is held as none: its
/// offsets move per draw.
#[derive(Default)]
pub struct DrawState {
    pipeline: Id,
    groups: [Id; BIND_GROUPS],
    lanes: [(Id, u32, u32); LANES],
    idx: (Id, u32, u32),
}

impl DrawState {
    pub fn is_pipeline_set(&self, pipeline: Id) -> bool {
        self.pipeline == pipeline
    }

    pub fn set_pipeline(&mut self, pipeline: Id) {
        self.pipeline = pipeline;
    }

    pub fn is_bind_group_set(&self, idx: u32, group: Id, offsets: &[u32]) -> bool {
        offsets.is_empty() && self.groups[idx as usize] == group
    }

    pub fn set_bind_group(&mut self, idx: u32, group: Id, offsets: &[u32]) {
        self.groups[idx as usize] = if offsets.is_empty() { group } else { 0 };
    }

    pub fn is_vertex_buf_set(&self, slot: u32, buf: Id, off: u32, size: u32) -> bool {
        self.lanes[slot as usize] == (buf, off, size)
    }

    pub fn set_vertex_buf(&mut self, slot: u32, buf: Id, off: u32, size: u32) {
        self.lanes[slot as usize] = (buf, off, size);
    }

    pub fn is_idx_buf_set(&self, buf: Id, off: u32, size: u32) -> bool {
        self.idx == (buf, off, size)
    }

    pub fn set_idx_buf(&mut self, buf: Id, off: u32, size: u32) {
        self.idx = (buf, off, size);
    }
}

/// A render pass handed from draw to draw, which sets no pipeline, bind
/// group, vertex lane or index buffer its `DrawState` says it holds. Every
/// call on the pass goes through it, so what it holds is what the pass has.
pub struct TrackedRenderPass<'a> {
    g: &'a Gpu,
    pass: Id,
    state: DrawState,
}

impl<'a> TrackedRenderPass<'a> {
    pub fn new(g: &'a Gpu, pass: Id) -> Self {
        Self {
            g,
            pass,
            state: DrawState::default(),
        }
    }

    pub fn set_pipeline(&mut self, pipeline: Id) {
        if !self.state.is_pipeline_set(pipeline) {
            self.g.set_pipeline(self.pass, pipeline);
            self.state.set_pipeline(pipeline);
        }
    }

    pub fn set_bind_group(&mut self, idx: u32, group: Id, offsets: &[u32]) {
        if !self.state.is_bind_group_set(idx, group, offsets) {
            self.g.set_bind_group(self.pass, idx, group, offsets);
            self.state.set_bind_group(idx, group, offsets);
        }
    }

    /// A size of 0 is the rest of the buffer.
    pub fn set_vertex_buf(&mut self, slot: u32, buf: Id, off: u32, size: u32) {
        if !self.state.is_vertex_buf_set(slot, buf, off, size) {
            self.g.set_vertex_buf(self.pass, slot, buf, off, size);
            self.state.set_vertex_buf(slot, buf, off, size);
        }
    }

    /// Indices are u32.
    pub fn set_idx_buf(&mut self, buf: Id, off: u32, size: u32) {
        if !self.state.is_idx_buf_set(buf, off, size) {
            self.g.set_idx_buf(self.pass, buf, off, size);
            self.state.set_idx_buf(buf, off, size);
        }
    }

    /// What the pass draws held to `[x, y, width, height]` of its targets.
    pub fn set_scissor(&self, rect: [u32; 4]) {
        self.g.set_scissor(self.pass, rect);
    }

    /// The stencil's reference its draws test and write (`Stencil`).
    pub fn set_stencil_reference(&self, reference: u32) {
        call::set_stencil_reference(&mut self.g.s().stream, self.pass, reference);
    }

    pub fn draw(&self, vertices: u32, instances: u32, first_vertex: u32, first_instance: u32) {
        self.g
            .draw(self.pass, vertices, instances, first_vertex, first_instance);
    }

    pub fn draw_indexed(&self, indices: u32, instances: u32, first_idx: u32, first_instance: u32) {
        self.g
            .draw_indexed(self.pass, indices, instances, first_idx, first_instance);
    }

    pub fn draw_indirect(&self, buf: Id, off: u32) {
        self.g.draw_indirect(self.pass, buf, off);
    }

    pub fn draw_indexed_indirect(&self, buf: Id, off: u32) {
        self.g.draw_indexed_indirect(self.pass, buf, off);
    }

    /// Ends the pass and lets its handle go.
    pub fn end(self) {
        self.g.end(self.pass);
    }
}

/// The depth a pass drawn through a gate draws into, the world's surfaces'
/// and the light's map's: a 32-bit float, and a stencil, which marks the
/// tiles a frame draws again, so a pass skips the rest (`Stencil`).
pub const GATED_DEPTH: Format = Format::depth32float_stencil8;

/// What a pipeline does with the stencil beside its depth (`GATED_DEPTH`),
/// which holds in each tile the number of the gated frame that last claimed
/// it, as a light tile's word names the cast that read it: nothing; passes
/// only where it holds the pass's reference, this frame's number, as every
/// draw of the world's pass does; or writes the reference there, as the gate
/// does (gpu/renderer.rs `frame`). A pass clears it to 0, its reference 0,
/// unless it keeps it (`PassDesc::load_stencil`), so a pass with no gate passes
/// everywhere.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Stencil {
    Unused,
    Gated,
    Gate,
}

/// How an attachment starts: cleared, kept, or (depth) read and not written.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Load {
    Clear,
    Load,
    ReadOnly,
}

/// A pass's targets: color cleared to transparent, or to what `color_clear`
/// gives, or loaded, depth cleared to `depth_clear`, loaded or read only, and
/// its stencil, where it has one, cleared to 0 or loaded (`load_stencil`); 0
/// for one absent.
pub struct PassDesc {
    color: Id,
    color_load: Load,
    color_clear: [f32; 4],
    depth: Id,
    depth_load: Load,
    depth_clear: f32,
    stencil_load: Load,
    stamp: Option<encode::TimestampWrites>,
}

impl PassDesc {
    /// Its stencil loaded as the last pass left it, the numbers of the gated
    /// frames that claimed each tile (`Stencil`).
    pub fn load_stencil(self) -> PassDesc {
        PassDesc {
            stencil_load: Load::Load,
            ..self
        }
    }

    /// Its color cleared to `rgba`, linear.
    pub fn color_clear(self, rgba: [f32; 4]) -> PassDesc {
        PassDesc {
            color_clear: rgba,
            ..self
        }
    }
}

pub fn pass_desc(
    color: Id,
    color_load: Load,
    depth: Id,
    depth_load: Load,
    depth_clear: f32,
    stamp: Option<encode::TimestampWrites>,
) -> PassDesc {
    PassDesc {
        color,
        color_load,
        color_clear: [0.0; 4],
        depth,
        depth_load,
        depth_clear,
        stencil_load: Load::Clear,
        stamp,
    }
}

/// A bind group layout, and the group it was made of: its (binding, stages, kind) words, as a
/// pipeline record holds them (frame/shader/record.rs), or a compute group. Made by
/// `Gpu::layout` alone, so a bind group over it is checked against the group itself.
#[derive(Clone, Copy, Default)]
pub struct Layout {
    id: Id,
    group: &'static [u32],
}

impl Layout {
    /// Whether the group has `binding`.
    pub fn binds(&self, binding: u32) -> bool {
        self.group
            .as_chunks::<3>()
            .0
            .iter()
            .any(|w| w[0] == binding)
    }

    /// The buffers of `entries` bound as writable storage.
    fn writable<'e>(&self, entries: &'e [Entry]) -> impl Iterator<Item = Id> + 'e {
        let words = self.group.as_chunks::<3>().0;
        entries.iter().filter_map(move |e| match e.resource {
            encode::Resource::Buffer { buffer, .. }
                if words
                    .iter()
                    .any(|w| w[0] == e.binding && w[2] == record::STORAGE_RW) =>
            {
                Some(buffer)
            }
            _ => None,
        })
    }

    /// `entries` the group's bindings each once, each a resource its kind takes (a buffer for
    /// a buffer's kind, a sampler for a sampler's, a view for the rest); a bug otherwise.
    #[track_caller]
    fn holds(&self, entries: &[Entry]) {
        let words = self.group.as_chunks::<3>().0;
        xpute_kit::ensure!(
            entries.len() == words.len(),
            ENOTRECOVERABLE,
            entries.len(),
            words.len()
        );
        for (k, e) in entries.iter().enumerate() {
            let [_, _, kind] = *words
                .iter()
                .find(|w| w[0] == e.binding)
                .or_bug(Errno::ENOTRECOVERABLE);
            let buf = matches!(
                kind,
                record::UNIFORM | record::UNIFORM_DYNAMIC | record::STORAGE | record::STORAGE_RW
            );
            let sampler = matches!(kind, record::SAMPLER | record::SAMPLER_COMPARISON);
            let takes = match e.resource {
                encode::Resource::Buffer { .. } => buf,
                encode::Resource::Sampler(_) => sampler,
                encode::Resource::View(_) => !buf && !sampler,
            };
            xpute_kit::ensure!(takes, ENOTRECOVERABLE, e.binding, kind);
            xpute_kit::ensure!(
                entries[..k].iter().all(|d| d.binding != e.binding),
                ENOTRECOVERABLE,
                e.binding
            );
        }
    }
}

/// A bind group's entry, as xpute's encoder takes it.
pub use xpute_gpu::encode::BindGroupEntry as Entry;

/// A buffer's entry, whole.
pub const fn buf_entry(binding: u32, buf: Id) -> Entry {
    Entry {
        binding,
        resource: encode::Resource::Buffer {
            buffer: buf,
            offset: 0,
            size: None,
        },
    }
}

/// `size` bytes of a buffer from `offset`.
pub const fn buf_range_entry(binding: u32, buf: Id, off: u32, size: u32) -> Entry {
    Entry {
        binding,
        resource: encode::Resource::Buffer {
            buffer: buf,
            offset: off,
            size: Some(size),
        },
    }
}

pub const fn view_entry(binding: u32, view: Id) -> Entry {
    Entry {
        binding,
        resource: encode::Resource::View(view),
    }
}

pub const fn sampler_entry(binding: u32, sampler: Id) -> Entry {
    Entry {
        binding,
        resource: encode::Resource::Sampler(sampler),
    }
}

pub fn bytes<T: Copy>(lane: &[T]) -> &[u8] {
    // SAFETY: plain numbers read as their bytes, for as long as the lane is borrowed.
    unsafe { core::slice::from_raw_parts(lane.as_ptr().cast::<u8>(), size_of_val(lane)) }
}

pub const fn word_bytes(words: usize) -> u32 {
    (words * 4) as u32
}

struct State {
    stream: Stream,
    /// At the range's end, past the stream: never built or moved, as all-zero
    /// bytes are an encoder that has made nothing (xpute-gpu encode).
    encoder: &'static mut Encoder<OBJECTS>,
    /// Where the stream's words begin in the kernel's memory.
    at: u32,
    /// Inside a frame's transaction, where a record that does not fit drops
    /// the frame rather than being a bug.
    in_frame: bool,
    /// The device's block asked for; read once it is READY.
    device_out: bool,
    device: Option<Device>,
    /// The spans' query set, its resolve buffer and the buffers it is copied
    /// into and mapped back through, a read's each; which spans each read
    /// timed, and the order it was asked in, 0 where none is out, so the
    /// reads are taken in as they were asked; and their times from the last
    /// read in, in ms.
    stamps: Option<(Id, Id, [Id; READS])>,
    stamps_written: [u64; READS],
    stamps_out: [u32; READS],
    stamps_asked_cnt: u32,
    /// The read this frame asked: its mapping's ticket is written as it is
    /// encoded, so a frame dropped whole takes it back (`frame_commit`), or
    /// it would never land.
    stamps_asked: Option<usize>,
    gpu_ms: [f64; SPANS],
    /// The time the last read's spans ran, ms, those that overlap counted
    /// once: the frame's GPU time, as the spans' sum is not where its passes
    /// overlap; a span whose ticks a pass left unwritten (a pass with no work
    /// may be dropped) adds its own length, not the time since.
    gpu_busy_ms: f64,
    /// The read the last frame submitted took in, where it took one: its
    /// figures are then that read's, and else the read before it, not
    /// measured again.
    gpu_landed: Option<usize>,
    /// The images on their way into textures (`Gpu::image`).
    images: Vec<Arriving>,
    /// The images taken since `take_landed` last ran, on this device.
    landed: Vec<u32>,
    /// The devices gone so far: what was handed to the GPU under another
    /// count is on none there is.
    generation: u32,
    /// The objects let go while the host held the stream, destroyed once it
    /// hands it back (`writable`).
    frees: Vec<Id>,
    /// A device answered while the host held the stream: its block is asked
    /// once it hands it back.
    device_pending: bool,
    /// The device's block said LOST when last read: the host alone writes
    /// the block, so a loss is taken once by this, not by clearing it.
    lost: bool,
    /// The ranges each WRITE_BUFFER_AT names, address and bytes: those
    /// recorded since the last publish, and those of the hand-off the host
    /// holds until it gives the stream back (`in_flight`).
    ranges: Vec<(usize, usize)>,
    ranges_out: Vec<(usize, usize)>,
    /// Where each encoder is cut into command buffers (gpu/split.rs).
    splits: Splits,
}

/// An image file copied into its texture: the file's bytes kept until the
/// texture's fence says it is there, or refused. Each asks once, its
/// ticket the first of its word's sequence (`IMAGE_TICKET`).
struct Arriving {
    image: u32,
    texture: Id,
    fence: Box<[u32; 2]>,
    /// Whether its landing was told (`images_landed`).
    told: bool,
}

const IMAGE_TICKET: u32 = 1;

static STATE: Global<Option<State>> = Global::new();

pub struct Gpu(());

static GPU: Gpu = Gpu(());

fn state() -> &'static mut State {
    STATE.get(|| None).as_mut().or_bug(Errno::ENODEV)
}

fn head(k: usize) -> &'static mut u32 {
    let at = state().at as usize + k * WORD;
    // SAFETY: a word of the stream's head, the kernel's and the host's alone.
    unsafe { &mut *mem::at::<u32>(at) }
}

/// Read `r`'s fence and result, the host's to write.
fn read_fence(r: usize) -> &'static [u32; 2] {
    let at = state().at as usize + read_at(r) * WORD;
    // SAFETY: two words of the stream's head, which the kernel only reads.
    unsafe { &*mem::at::<[u32; 2]>(at) }
}

/// The encoder's bytes at the range's end, a word-multiple of 8.
const ENCODER_BYTES: usize = size_of::<Encoder<OBJECTS>>().next_multiple_of(8);
const _: () =
    assert!(align_of::<Encoder<OBJECTS>>() <= 8 && ENCODER_BYTES < stream::RESERVE as usize);

/// # Safety
/// `at` must address `bytes`, zeroed, that nothing else writes, 8-aligned,
/// for as long as the module runs: the stream, then the encoder's handle
/// table at its end.
pub unsafe fn install(at: u32, bytes: u32) -> u32 {
    xpute_kit::ensure!(
        bytes as usize > ENCODER_BYTES + stream::RESERVE as usize,
        EINVAL,
        bytes
    );
    let words = (bytes as usize - ENCODER_BYTES) / WORD;
    // SAFETY: the caller's range: the stream, then the encoder, which zeroed
    // bytes are an empty one of, aligned as the range is.
    let (stream, encoder) = unsafe {
        (
            Stream::new(mem::core(), at as usize, words, HEAD),
            &mut *mem::at::<Encoder<OBJECTS>>(at as usize + bytes as usize - ENCODER_BYTES),
        )
    };
    STATE.set(Some(State::over(stream, encoder, at)));
    at
}

/// The calls' state over a stream in a region of its own, apart from the kernel's memory, its
/// encoder beside it, and `device` as though its block were in: what a test makes the renderer
/// on, no host running the stream.
#[cfg(test)]
pub(crate) fn install_apart(device: Device) {
    const WORDS: usize = 1 << 25;
    let region: &'static mut [u64] = Vec::leak(vec![0; WORDS / 2]);
    // SAFETY: leaked, so it lives as long as the process, and nothing else names it.
    let stream = unsafe {
        let mem = mem::Memory::new(region.as_mut_ptr().expose_provenance(), WORDS * WORD);
        Stream::new(mem, 0, WORDS, HEAD)
    };
    // SAFETY: all-zero bytes are an encoder that has made nothing (xpute-gpu encode).
    let encoder = unsafe { Box::<Encoder<OBJECTS>>::new_zeroed().assume_init() };
    let mut s = State::over(stream, Box::leak(encoder), 0);
    s.device = Some(device);
    STATE.set(Some(s));
}

/// A device made up, as `install_apart` takes it.
#[cfg(test)]
pub(crate) fn test_device() -> Device {
    Device {
        align: UNIFORM_ALIGN_MAX,
        timestamps: false,
        canvas: Format::bgra8unorm,
        period_ns: 1.0,
    }
}

impl State {
    fn over(stream: Stream, encoder: &'static mut Encoder<OBJECTS>, at: u32) -> State {
        State {
            stream,
            encoder,
            at,
            in_frame: false,
            device_out: false,
            device: None,
            stamps: None,
            stamps_written: [0; READS],
            stamps_out: [0; READS],
            stamps_asked_cnt: 0,
            stamps_asked: None,
            gpu_ms: [0.0; SPANS],
            gpu_busy_ms: 0.0,
            gpu_landed: None,
            images: Vec::new(),
            landed: Vec::new(),
            generation: 0,
            frees: Vec::new(),
            device_pending: false,
            lost: false,
            ranges: Vec::new(),
            ranges_out: Vec::new(),
            splits: Splits::default(),
        }
    }
}

/// Where the stream lies in the kernel's memory, once installed: what a host
/// runs after each turn.
pub fn stream_at() -> Option<u32> {
    STATE.get(|| None).as_ref().map(|s| s.at)
}

/// The renderer's calls, once the device has told what it is, on a turn
/// the stream is the kernel's to write (`stream_free`).
pub fn gpu() -> Option<&'static Gpu> {
    device().filter(|_| stream_free()).map(|_| &GPU)
}

/// Whether the host has handed the stream back since the kernel last
/// published it (xpute_guest ipc/stream.rs R3): until then the kernel
/// records nothing, and a frame is held.
pub fn stream_free() -> bool {
    STATE.get(|| None).as_mut().is_none_or(writable)
}

/// `stream_free`, and once it is, what waited for it recorded: the device's
/// block asked and the objects let go meanwhile destroyed.
fn writable(s: &mut State) -> bool {
    if !s.stream.is_free() {
        return false;
    }
    if core::mem::take(&mut s.device_pending) {
        ask_device(s);
    }
    let State {
        frees,
        encoder,
        stream,
        ..
    } = s;
    for id in frees.drain(..) {
        encoder.destroy(stream, id);
    }
    true
}

/// The device as its block says, once the host has written it: asked for on
/// the first call after a device answers (`on_device_answered`).
pub fn device() -> Option<Device> {
    let s = STATE.get(|| None).as_mut()?;
    if s.device.is_none()
        && s.device_out
        && load_acquire(head(DEVICE_AT + DEVICE_WORDS - 1)) == STATUS::READY
    {
        let word = |k: usize| *head(DEVICE_AT + k);
        let limit = |name: &str| {
            word(
                LIMIT_NAME
                    .iter()
                    .position(|n| *n == name)
                    .or_bug(Errno::ENOENT),
            )
        };
        let feat = |name: &str| {
            let k = FEATURE_NAME
                .iter()
                .position(|n| *n == name)
                .or_bug(Errno::ENOENT);
            word(LIMIT_NAME.len() + k / 32) >> (k % 32) & 1 != 0
        };
        let canvas = *Format::ALL
            .get(word(PERIOD_AT + 1) as usize)
            .or_bug(Errno::EINVAL);
        let align = limit("minUniformBufferOffsetAlignment");
        xpute_kit::ensure!(
            align.is_power_of_two() && align <= UNIFORM_ALIGN_MAX,
            EINVAL,
            align
        );
        s.device = Some(Device {
            align,
            timestamps: feat("timestamp-query"),
            canvas,
            period_ns: f32::from_bits(word(PERIOD_AT)),
        });
    }
    s.device
}

/// A device answered: its block is asked for, once the stream is the
/// kernel's to write, and read once the host has written it.
pub fn on_device_answered() {
    let s = state();
    if writable(s) {
        ask_device(s);
    } else {
        s.device_pending = true;
    }
}

fn ask_device(s: &mut State) {
    let at = s.at as usize + DEVICE_AT * WORD;
    // SAFETY: the head's block, the host's to write.
    let block = unsafe { core::slice::from_raw_parts_mut(mem::at::<u32>(at), DEVICE_WORDS) };
    call::device(&mut s.stream, block);
    s.device_out = true;
}

/// Whether the device went since this was last asked, as its host says in
/// the device's block (xpute STATUS.LOST); taken once, the block left as
/// the host wrote it.
pub fn take_device_lost() -> bool {
    let Some(s) = STATE.get(|| None).as_mut() else {
        return false;
    };
    let lost = load_acquire(head(DEVICE_AT + DEVICE_WORDS - 1)) == STATUS::LOST;
    !core::mem::replace(&mut s.lost, lost) && lost
}

/// The devices gone so far (`on_device_lost`): whoever hands the GPU anything
/// says under which, and what went under another is not there.
pub fn device_generation() -> u32 {
    STATE.get(|| None).as_ref().map_or(0, |s| s.generation)
}

/// The ticks of the read asked first of those out, where its word says they
/// landed: each span's own ms, and the read let go for another frame to
/// ask; which read landed. One a frame is taken, as one a frame is asked.
fn take_stamps(s: &mut State) -> Option<usize> {
    let r = (0..READS)
        .filter(|&r| s.stamps_out[r] != 0)
        .min_by_key(|&r| s.stamps_out[r])?;
    let fence = read_fence(r);
    if !signaled(&fence[0], s.stamps_out[r]) {
        return None;
    }
    s.stamps_out[r] = 0;
    if fence[1] != RESULT::OK {
        return None;
    }
    let written = s.stamps_written[r];
    let period_ms = f64::from(s.device.map_or(1.0, |d| d.period_ns)) / 1e6;
    let ticks = read_at(r) + 2;
    let tick =
        |k: usize| u64::from(*head(ticks + 2 * k)) | u64::from(*head(ticks + 2 * k + 1)) << 32;
    let mut ran = [(0u64, 0u64); SPANS];
    for (k, r) in ran.iter_mut().enumerate() {
        let (b, e) = (tick(2 * k), tick(2 * k + 1));
        if written & 1u64 << k != 0 && e >= b {
            *r = (b, e);
        }
    }
    // A span's own time is its end less the later of its begin and the
    // latest end before it: a render pass's begin is stamped before the
    // passes it waits on have finished (a tiled GPU starts its vertex work
    // early), while its end and a compute pass's begin are not, so the end
    // before it bounds what it took of their time.
    let ends = ran;
    for (k, r) in ran.iter_mut().enumerate() {
        let (b, e) = *r;
        let before = ends
            .iter()
            .enumerate()
            .filter(|&(j, &(_, ej))| j != k && ej != 0 && ej < e)
            .map(|(_, &(_, ej))| ej)
            .max();
        r.0 = before.map_or(b, |p| p.clamp(b, e));
        s.gpu_ms[k] = (r.1 - r.0) as f64 * period_ms;
    }
    // The spans' own times by their starts, each run of overlapping ones
    // counted once.
    let mut cnt = 0;
    for k in 0..SPANS {
        if ran[k].1 != 0 {
            ran[cnt] = ran[k];
            cnt += 1;
        }
    }
    let ran = &mut ran[..cnt];
    ran.sort_unstable();
    let mut busy = 0;
    let mut run: Option<(u64, u64)> = None;
    for &(b, e) in ran.iter() {
        run = match run {
            Some((rb, re)) if b <= re => Some((rb, re.max(e))),
            Some((rb, re)) => {
                busy += re - rb;
                Some((b, e))
            }
            None => Some((b, e)),
        };
    }
    if let Some((rb, re)) = run {
        busy += re - rb;
    }
    s.gpu_busy_ms = busy as f64 * period_ms;
    Some(r)
}

/// The device went, and every object on it: the handles start over.
pub fn on_device_lost() {
    if let Some(s) = STATE.get(|| None).as_mut() {
        // SAFETY: the encoder's own bytes; zeroed, it has made nothing.
        unsafe { core::ptr::write_bytes(core::ptr::from_mut(s.encoder), 0, 1) };
        (s.device_out, s.device, s.stamps) = (false, None, None);
        s.images.clear();
        s.landed.clear();
        s.generation = s.generation.wrapping_add(1);
        s.frees.clear();
        s.device_pending = false;
        s.stamps_out = [0; READS];
    }
}

/// The turn's records handed to the host, and with them the ranges they
/// name (`in_flight`).
pub fn gpu_publish() {
    if let Some(s) = STATE.get(|| None).as_mut() {
        if s.stream.is_free() {
            core::mem::swap(&mut s.ranges_out, &mut s.ranges);
            s.ranges.clear();
        }
        s.stream.publish();
    }
}

/// Whether the host holds memory of `vals`: a WRITE_BUFFER_AT names it, of
/// the records not yet published or of the hand-off the host has not given
/// back (xpute_guest ipc/stream.rs R3). Their writer leaves them as they
/// are, and where they are, until neither does.
pub fn in_flight<T>(vals: &[T]) -> bool {
    let Some(s) = STATE.get(|| None).as_mut() else {
        return false;
    };
    let (at, end) = (
        vals.as_ptr() as usize,
        vals.as_ptr() as usize + size_of_val(vals),
    );
    let meets = |ranges: &[(usize, usize)]| ranges.iter().any(|&(a, n)| a < end && at < a + n);
    meets(&s.ranges) || (!s.stream.is_free() && meets(&s.ranges_out))
}

/// Each image whose texture was taken since this last ran, on the device
/// there is: its asker may let its file go, and hands it again only once
/// the device is lost.
pub fn take_landed(mut landed: impl FnMut(u32)) {
    if let Some(s) = STATE.get(|| None).as_mut() {
        s.landed.drain(..).for_each(&mut landed);
    }
}

/// Whether an image handed to the device landed since this last ran
/// (`Gpu::image`): its fence's store woke the turn, and a frame is wanted
/// to take it.
pub fn images_landed() -> bool {
    STATE.get(|| None).as_mut().is_some_and(|s| {
        s.images.iter_mut().fold(false, |any, a| {
            let landed = !a.told && signaled(&a.fence[0], IMAGE_TICKET);
            a.told |= landed;
            any | landed
        })
    })
}

/// A frame's records are one: past the range they are dropped whole, with
/// the objects they made.
pub fn frame_begin() {
    let s = state();
    s.encoder.begin(&mut s.stream);
    s.in_frame = true;
    s.stamps_asked = None;
}

/// False where the frame did not fit and nothing of it was recorded: the
/// read it asked, never mapped, is freed for the next frame to ask.
pub fn frame_commit() -> bool {
    let s = state();
    s.in_frame = false;
    let committed = s.encoder.commit(&mut s.stream);
    if let Some(r) = s.stamps_asked.take().filter(|_| !committed) {
        s.stamps_out[r] = 0;
    }
    committed
}

use crate::frame::shader::floor;
use pxl_spec::asset;
/// The bytes a write of `bytes` takes of the stream, as its writer measures
/// it: what a caller sums over the writes it makes together and asks `room`.
pub use xpute_gpu::encode::call::{write_buffer_bytes, write_texture_bytes};

/// What a shader module of `code` takes of the stream (`Gpu::shader`), as
/// its writer measures it: what its maker asks `room` before it.
pub fn shader_bytes(code: &str) -> usize {
    Encoder::<OBJECTS>::shader_module_bytes(code)
}

/// Whether records of `bytes` (`write_buffer_bytes`, `write_texture_bytes`,
/// `shader_bytes`) fit the stream, leaving `stream::RESERVE` for what cannot
/// wait.
pub fn room(bytes: usize) -> bool {
    STATE
        .get(|| None)
        .as_ref()
        .is_some_and(|s| s.stream.fits(bytes, stream::RESERVE as usize))
}

/// An object's handle, or 0 where the frame it was made in no longer fits
/// (and is dropped with it); a full table is a bug.
fn handle_of(id: Option<Id>) -> Id {
    match id {
        Some(id) => id,
        None => {
            xpute_kit::ensure!(state().in_frame, ENOSPC, OBJECTS);
            0
        }
    }
}

/// Lets an object go: the host destroys it, and its handle is free. 0 is nothing.
pub fn release(id: Id) {
    if id != 0 {
        if let Some(s) = STATE.get(|| None).as_mut() {
            s.splits.let_go(id);
            if writable(s) {
                s.encoder.destroy(&mut s.stream, id);
            } else {
                s.frees.push(id);
            }
        }
    }
}

/// A record's vertex format code as WebGPU's.
pub fn vertex_format(code: u32) -> VertexFormat {
    match code {
        record::F32X2 => VertexFormat::float32x2,
        record::F32X3 => VertexFormat::float32x3,
        record::F32X4 => VertexFormat::float32x4,
        record::U32X4 => VertexFormat::uint32x4,
        record::U32X2 => VertexFormat::uint32x2,
        record::U32 => VertexFormat::uint32,
        _ => xpute_kit::bug!(EINVAL, code),
    }
}

fn binding_layout(kind: u32) -> encode::BindingLayout {
    use encode::BindingLayout as L;
    let buf = |ty, dynamic_off| L::Buffer {
        ty,
        dynamic_offset: dynamic_off,
        min_binding_size: 0,
    };
    let texture = |sample, dim| L::Texture {
        sample,
        dimension: dim,
        multisampled: false,
    };
    match kind {
        record::UNIFORM => buf(BufferBindingType::uniform, false),
        record::UNIFORM_DYNAMIC => buf(BufferBindingType::uniform, true),
        record::STORAGE => buf(BufferBindingType::read_only_storage, false),
        record::STORAGE_RW => buf(BufferBindingType::storage, false),
        record::TEXTURE => texture(TextureSampleType::float, TextureViewDimension::_2d),
        record::TEXTURE_ARRAY => texture(TextureSampleType::float, TextureViewDimension::_2d_array),
        record::DEPTH => texture(TextureSampleType::depth, TextureViewDimension::_2d),
        record::TEXTURE_UNFILTERED => texture(
            TextureSampleType::unfilterable_float,
            TextureViewDimension::_2d,
        ),
        record::TEXTURE_UINT_ARRAY => {
            texture(TextureSampleType::uint, TextureViewDimension::_2d_array)
        }
        record::STORAGE_TEXTURE_RW => L::StorageTexture {
            access: StorageTextureAccess::read_write,
            format: TextureFormat::r32uint,
            dimension: TextureViewDimension::_2d_array,
        },
        record::TEXTURE_3D => texture(TextureSampleType::float, TextureViewDimension::_3d),
        record::STORAGE_TEXTURE_3D => L::StorageTexture {
            access: StorageTextureAccess::write_only,
            format: TextureFormat::rgba16float,
            dimension: TextureViewDimension::_3d,
        },
        record::STORAGE_TEXTURE_2D => L::StorageTexture {
            access: StorageTextureAccess::write_only,
            format: TextureFormat::rgba16float,
            dimension: TextureViewDimension::_2d,
        },
        record::SAMPLER_COMPARISON => L::Sampler(SamplerBindingType::comparison),
        _ => L::Sampler(SamplerBindingType::filtering),
    }
}

const fn comp(
    operation: BlendOperation,
    from: BlendFactor,
    to: BlendFactor,
) -> encode::BlendComponent {
    encode::BlendComponent {
        operation,
        src: from,
        dst: to,
    }
}

const ALPHA_BLEND: encode::BlendState = encode::BlendState {
    color: comp(
        BlendOperation::add,
        BlendFactor::src_alpha,
        BlendFactor::one_minus_src_alpha,
    ),
    alpha: comp(
        BlendOperation::add,
        BlendFactor::one,
        BlendFactor::one_minus_src_alpha,
    ),
};

const ADD_BLEND: encode::BlendState = encode::BlendState {
    color: comp(BlendOperation::add, BlendFactor::one, BlendFactor::one),
    alpha: comp(BlendOperation::add, BlendFactor::zero, BlendFactor::one),
};

fn load_op(l: Load) -> LoadOp {
    if l == Load::Load {
        LoadOp::load
    } else {
        LoadOp::clear
    }
}

/// A face's stencil state as `stencil` asks: a draw passing where the
/// stencil holds the reference, the gate writing it.
fn stencil_face(stencil: Stencil) -> encode::StencilFaceState {
    match stencil {
        Stencil::Unused => encode::STENCIL_UNUSED,
        Stencil::Gated => encode::StencilFaceState {
            compare: CompareFunction::equal,
            ..encode::STENCIL_UNUSED
        },
        Stencil::Gate => encode::StencilFaceState {
            pass: StencilOperation::replace,
            ..encode::STENCIL_UNUSED
        },
    }
}

impl Gpu {
    fn s(&self) -> &'static mut State {
        state()
    }

    /// A buffer of `size` bytes, one every device makes and, as storage,
    /// binds (frame/shader/floor.rs `fits`): a size past that is a bug the
    /// structure let through, not a device to fall short on.
    pub fn buf(&self, size: u32, usage: u32) -> Id {
        xpute_kit::ensure!(
            floor::fits(u64::from(size), usage & buffer_usage::STORAGE != 0),
            ENOSPC,
            size
        );
        let s = self.s();
        let buf = handle_of(s.encoder.buffer(
            &mut s.stream,
            &encode::BufferDescriptor {
                size,
                usage,
                mapped_at_creation: false,
            },
        ));
        s.splits.made(buf, usage & buffer_usage::INDIRECT != 0);
        buf
    }

    pub fn texture(
        &self,
        width: u32,
        height: u32,
        layers: u32,
        format: Format,
        mips: u32,
        usage: u32,
    ) -> Id {
        let s = self.s();
        handle_of(s.encoder.texture(
            &mut s.stream,
            &encode::TextureDescriptor {
                size: [width, height, layers],
                mip_levels: mips,
                samples: 1,
                dimension: TextureDimension::_2d,
                format,
                usage,
                view_formats: &[],
            },
        ))
    }

    /// A volume `size` texels on a side each way: no mips, no layers.
    pub fn volume(&self, size: [u32; 3], format: Format, usage: u32) -> Id {
        let s = self.s();
        handle_of(s.encoder.texture(
            &mut s.stream,
            &encode::TextureDescriptor {
                size,
                mip_levels: 1,
                samples: 1,
                dimension: TextureDimension::_3d,
                format,
                usage,
                view_formats: &[],
            },
        ))
    }

    /// A volume's view, whole (`volume`).
    pub fn volume_view(&self, texture: Id) -> Id {
        let s = self.s();
        handle_of(s.encoder.view(
            &mut s.stream,
            texture,
            &encode::ViewDescriptor {
                dimension: Some(TextureViewDimension::_3d),
                base_mip: 0,
                mips: Some(1),
                base_layer: 0,
                layers: Some(1),
                ..Default::default()
            },
        ))
    }

    /// `array` for a view of the texture's layers as one array.
    pub fn view(
        &self,
        texture: Id,
        array: bool,
        base_mip: u32,
        mips: u32,
        base_layer: u32,
        layers: u32,
    ) -> Id {
        let s = self.s();
        handle_of(s.encoder.view(
            &mut s.stream,
            texture,
            &encode::ViewDescriptor {
                dimension: Some(if array {
                    TextureViewDimension::_2d_array
                } else {
                    TextureViewDimension::_2d
                }),
                base_mip,
                mips: Some(mips),
                base_layer,
                layers: Some(layers),
                ..Default::default()
            },
        ))
    }

    /// The depth alone of a depth target that carries a stencil, as a
    /// binding reads it: WebGPU binds no view of both aspects.
    pub fn depth_only_view(&self, texture: Id) -> Id {
        let s = self.s();
        handle_of(s.encoder.view(
            &mut s.stream,
            texture,
            &encode::ViewDescriptor {
                dimension: Some(TextureViewDimension::_2d),
                aspect: Some(TextureAspect::depth_only),
                mips: Some(1),
                layers: Some(1),
                ..Default::default()
            },
        ))
    }

    /// Both directions addressed alike; `compare` for a comparison sampler.
    pub fn sampler(
        &self,
        mag: FilterMode,
        min: FilterMode,
        mipmap: MipmapFilterMode,
        addr: AddressMode,
        compare: Option<CompareFunction>,
        anisotropy: u32,
    ) -> Id {
        let s = self.s();
        handle_of(s.encoder.sampler(
            &mut s.stream,
            &encode::SamplerDescriptor {
                address: [addr, addr, AddressMode::clamp_to_edge],
                mag,
                min,
                mipmap,
                lod: [0.0, 32.0],
                compare,
                max_anisotropy: anisotropy.max(1),
            },
        ))
    }

    pub fn shader(&self, code: &str) -> Id {
        let s = self.s();
        handle_of(s.encoder.shader_module(&mut s.stream, code))
    }

    /// A group's layout from its (binding, stages, kind) words, as a pipeline record holds them.
    pub fn layout(&self, group: &'static [u32]) -> Layout {
        let words = group.as_chunks::<3>().0;
        xpute_kit::ensure!(words.len() <= GROUP_ENTRIES, ENOSPC, words.len());
        let entries: [encode::BindGroupLayoutEntry; GROUP_ENTRIES] = core::array::from_fn(|i| {
            let [binding, stage, kind] = words.get(i).copied().unwrap_or_default();
            encode::BindGroupLayoutEntry {
                binding,
                visibility: stage,
                layout: binding_layout(kind),
            }
        });
        let s = self.s();
        let id = handle_of(
            s.encoder
                .bind_group_layout(&mut s.stream, &entries[..words.len()]),
        );
        Layout { id, group }
    }

    fn pipeline_layout(&self, layouts: &[Layout]) -> Id {
        let ids: Vec<Id> = layouts.iter().map(|l| l.id).collect();
        let s = self.s();
        handle_of(s.encoder.pipeline_layout(&mut s.stream, &ids))
    }

    /// A render pipeline from its record (frame/shader/record.rs) over the
    /// group layouts, drawing into `format` and `depth`, `stencil` the
    /// stencil beside it: into the depth alone where `format` is depth32's,
    /// its fragment writing no color (the shadow map, gpu/shadow.rs). Made by
    /// WebGPU's async call: the host stores `ticket` at the fence once it
    /// is made, and nothing names it before (gpu/pipeline.rs `Pipelines::made`).
    pub fn pipeline(
        &self,
        words: &[u32],
        layouts: &[Layout],
        module: Id,
        format: Format,
        (depth, stencil): (Format, Stencil),
        (fence, ticket): (&u32, u32),
    ) -> Id {
        let mut at = record::HEAD as usize;
        let mut next = || {
            at += 1;
            words[at - 1]
        };
        let lanes: Vec<(u32, VertexStepMode, Vec<encode::VertexAttribute>)> = (0..next())
            .map(|_| {
                let stride = next();
                let step = if next() == record::PER_INSTANCE {
                    VertexStepMode::instance
                } else {
                    VertexStepMode::vertex
                };
                let attributes = (0..next())
                    .map(|_| {
                        let location = next();
                        let off = next();
                        encode::VertexAttribute {
                            format: vertex_format(next()),
                            offset: off,
                            location,
                        }
                    })
                    .collect();
                (stride, step, attributes)
            })
            .collect();
        let buffers: Vec<encode::VertexBufferLayout> = lanes
            .iter()
            .map(|(stride, step, attributes)| encode::VertexBufferLayout {
                stride: *stride,
                step: *step,
                attributes,
            })
            .collect();
        let layout = self.pipeline_layout(layouts);
        let blend = match words[8] {
            record::ALPHA => Some(ALPHA_BLEND),
            record::ADD => Some(ADD_BLEND),
            _ => None,
        };
        let targets = [encode::ColorTargetState {
            format,
            blend,
            write_mask: COLOR_WRITE::ALL,
        }];
        let compare = match words[5] {
            record::GREATER => CompareFunction::greater,
            record::GREATER_EQUAL => CompareFunction::greater_equal,
            record::LESS => CompareFunction::less,
            _ => CompareFunction::always,
        };
        let s = self.s();
        let p = handle_of(s.encoder.render_pipeline_async(
            &mut s.stream,
            &encode::RenderPipelineDescriptor {
                layout: Some(layout),
                vertex: encode::VertexState {
                    module,
                    entry: VERTEX_ENTRY[words[record::HEAD as usize - 1] as usize],
                    buffers: &buffers,
                },
                primitive: encode::PrimitiveState {
                    topology: match words[2] {
                        record::STRIP => PrimitiveTopology::triangle_strip,
                        _ => PrimitiveTopology::triangle_list,
                    },
                    strip_index_format: None,
                    front_face: FrontFace::ccw,
                    cull: if words[3] == record::CULL_BACK {
                        CullMode::back
                    } else {
                        CullMode::none
                    },
                    unclipped_depth: false,
                },
                depth_stencil: (words[5] != record::NO_DEPTH).then_some(
                    encode::DepthStencilState {
                        format: depth,
                        depth_write: words[4] != 0,
                        depth_compare: compare,
                        stencil_front: stencil_face(stencil),
                        stencil_back: stencil_face(stencil),
                        stencil_read_mask: u32::MAX,
                        stencil_write_mask: u32::MAX,
                        bias: words[6] as i32,
                        slope_scale: f32::from_bits(words[7]),
                        clamp: 0.0,
                    },
                ),
                multisample: encode::MULTISAMPLE_NONE,
                fragment: (words[1] != 0).then_some(encode::FragmentState {
                    module,
                    entry: FRAGMENT_ENTRY,
                    targets: if format == Format::depth32float {
                        &[]
                    } else {
                        &targets
                    },
                }),
            },
            fence,
            ticket,
        ));
        s.encoder.destroy(&mut s.stream, layout);
        p
    }

    /// The module's compute entry `entry`, over `layouts`, its fence as
    /// `pipeline`'s.
    pub fn compute_pipeline(
        &self,
        module: Id,
        entry: &str,
        layouts: &[Layout],
        (fence, ticket): (&u32, u32),
    ) -> Id {
        let layout = self.pipeline_layout(layouts);
        let s = self.s();
        let p = handle_of(s.encoder.compute_pipeline_async(
            &mut s.stream,
            &encode::ComputePipelineDescriptor {
                layout: Some(layout),
                module,
                entry,
            },
            fence,
            ticket,
        ));
        s.encoder.destroy(&mut s.stream, layout);
        p
    }

    /// A bind group over `layout`, `entries` its group's bindings each once, each a resource
    /// its kind takes: any other is a bug (`Layout::holds`).
    #[track_caller]
    pub fn bind_group(&self, layout: Layout, entries: &[Entry]) -> Id {
        layout.holds(entries);
        let s = self.s();
        let group = handle_of(s.encoder.bind_group(&mut s.stream, layout.id, entries));
        s.splits.grouped(group, layout.writable(entries));
        group
    }

    /// The format the device's surfaces show best, which the canvas's pipelines target.
    pub fn canvas(&self) -> Format {
        self.s().device.or_bug(Errno::ENODEV).canvas
    }

    /// `bytes` copied into the stream: any range, however soon its owner
    /// rewrites it.
    pub fn write_buf(&self, id: Id, off: u32, bytes: &[u8]) {
        call::write_buffer(&mut self.s().stream, id, off, bytes);
    }

    /// `bytes` read where they lie when the host runs the turn, one record
    /// however large: the host's from the turn's publish until it hands the
    /// stream back (xpute_guest ipc/stream.rs R3), their owner asking `in_flight`
    /// before it writes them again.
    pub fn upload_buf(&self, id: Id, off: u32, bytes: &[u8]) {
        let s = self.s();
        if call::write_buffer_at(&mut s.stream, id, off, bytes) {
            s.ranges.push((bytes.as_ptr() as usize, bytes.len()));
        }
    }

    /// One layer's first mip, whole, four bytes a texel.
    pub fn write_texture(&self, id: Id, layer: u32, width: u32, height: u32, bytes: &[u8]) {
        call::write_texture(
            &mut self.s().stream,
            &place(id, [0, 0, layer]),
            [width * 4, height],
            [width, height, 1],
            bytes,
        );
    }

    /// The image file at `file` (PNG, JPEG, WebP) decoded into `texture`,
    /// `ticket` landing at its fence `fence` once it is there or refused.
    pub fn copy_image(&self, texture: Id, file: &[u8], fence: &[u32; 2], ticket: u32) {
        let to = place(texture, [0, 0, 0]);
        call::copy_external_image(&mut self.s().stream, &to, false, false, file, fence, ticket);
    }

    /// The texture the image file `file`, named `image` by whoever hands it,
    /// is copied into, taken once it has landed: each image is copied in
    /// once, on this device, for the one that takes it. None while it
    /// lands, the first ask handing it to the device; none for good for a
    /// file whose size no header gives (pxl-spec `asset::image_size`),
    /// which the kernel does not hand, for one the host could not decode,
    /// and for the empty file of one already taken on this device.
    /// The file's bytes are its asker's until it is taken (`take_landed`);
    /// the host copies them out as it runs the turn.
    pub fn image(&self, image: u32, file: &'static [u8]) -> Option<Id> {
        let s = self.s();
        if let Some(k) = s.images.iter().position(|a| a.image == image) {
            let fence = &s.images[k].fence;
            if !signaled(&fence[0], IMAGE_TICKET) || fence[1] != RESULT::OK {
                return None;
            }
            s.landed.push(image);
            return Some(s.images.swap_remove(k).texture);
        }
        let (width, height) = asset::image_size(file)?;
        let usage = TEXTURE_USAGE::TEXTURE_BINDING
            | TEXTURE_USAGE::COPY_SRC
            | TEXTURE_USAGE::COPY_DST
            | TEXTURE_USAGE::RENDER_ATTACHMENT;
        let texture = self.texture(width, height, 1, Format::rgba8unorm_srgb, 1, usage);
        let fence = Box::new([0; 2]);
        self.copy_image(texture, file, &fence, IMAGE_TICKET);
        self.s().images.push(Arriving {
            image,
            texture,
            fence,
            told: false,
        });
        None
    }

    /// An encoder, cut into command buffers as its passes read args
    /// (gpu/split.rs) and submitted as them.
    pub fn encoder(&self) -> Id {
        let s = self.s();
        let encoder = handle_of(s.encoder.encoder(&mut s.stream));
        s.splits.begun(encoder);
        encoder
    }

    /// `encoder` recorded into a new command buffer from here (gpu/split.rs).
    fn split(&self, encoder: Id) {
        let s = self.s();
        let next = handle_of(s.encoder.encoder(&mut s.stream));
        s.splits.split(encoder, next);
    }

    /// The pass an indirect call on `pass` reading `buf` is recorded in: a
    /// compute pass is ended and begun again in a new command buffer where
    /// `buf` was written in its own (`Splits::cuts`), its pipeline and
    /// groups set again and its span's end moved to it; a render pass began
    /// apart (`Splits::draws_apart`), so its own draws alone could have
    /// written it, a bug.
    #[track_caller]
    fn indirect(&self, pass: Id, buf: Id) -> Id {
        let s = self.s();
        if !s.splits.cuts(pass, buf) {
            return s.splits.pass_at(pass);
        }
        let (encoder, compute) = s.splits.pass_on(pass).or_bug(Errno::ENOTRECOVERABLE);
        xpute_kit::ensure!(compute, ENOTRECOVERABLE, pass, buf);
        let o = s.splits.open(pass).or_bug(Errno::ENOTRECOVERABLE);
        let (was, end, pipeline) = (o.at, o.end, o.pipeline);
        let groups = o.groups.clone();
        s.encoder.end(&mut s.stream, was);
        self.split(encoder);
        let s = self.s();
        let stamp = end.map(|(query_set, end)| encode::TimestampWrites {
            query_set,
            begin: None,
            end: Some(end),
        });
        let at = handle_of(
            s.encoder
                .compute_pass(&mut s.stream, s.splits.at(encoder), &stamp),
        );
        if pipeline != 0 {
            call::set_pipeline(&mut s.stream, at, pipeline);
        }
        for (idx, (group, offsets)) in groups.iter().enumerate() {
            if *group != 0 {
                call::set_bind_group(&mut s.stream, at, idx as u32, *group, offsets);
            }
        }
        s.splits.open(pass).or_bug(Errno::ENOTRECOVERABLE).at = at;
        at
    }

    /// `size` texels of `src` into `dst`, as WebGPU's `copyTextureToTexture`.
    pub fn copy_texture(
        &self,
        encoder: Id,
        src: &encode::TexelCopyTexture,
        dst: &encode::TexelCopyTexture,
        size: [u32; 3],
    ) {
        let s = self.s();
        let encoder = s.splits.at(encoder);
        call::copy_texture_to_texture(&mut s.stream, encoder, src, dst, size);
    }

    pub fn pass(&self, encoder: Id, d: &PassDesc) -> Id {
        let named = encoder;
        if self.s().splits.draws_apart(named) {
            self.split(named);
        }
        let encoder = self.s().splits.at(named);
        let colors = [encode::ColorAttachment {
            view: d.color,
            resolve: None,
            load: load_op(d.color_load),
            store: StoreOp::store,
            clear: d.color_clear,
        }];
        let depth_stencil = (d.depth != 0).then(|| encode::DepthStencilAttachment {
            view: d.depth,
            depth_load: load_op(d.depth_load),
            depth_store: StoreOp::store,
            depth_clear: d.depth_clear,
            depth_read_only: d.depth_load == Load::ReadOnly,
            stencil_load: load_op(d.stencil_load),
            stencil_store: StoreOp::store,
            stencil_clear: 0,
            stencil_read_only: false,
        });
        let s = self.s();
        let pass = handle_of(s.encoder.render_pass(
            &mut s.stream,
            encoder,
            &encode::RenderPassDescriptor {
                colors: if d.color != 0 { &colors } else { &[] },
                depth_stencil,
                timestamps: d.stamp.as_ref().map(|t| encode::TimestampWrites {
                    query_set: t.query_set,
                    begin: t.begin,
                    end: t.end,
                }),
                occlusion: None,
            },
        ));
        s.splits.opened(pass, named, false, None);
        pass
    }

    /// Timed in `stamp`'s span where the frame is timed.
    pub fn compute_pass(&self, encoder: Id, stamp: Option<encode::TimestampWrites>) -> Id {
        let s = self.s();
        let end = stamp.as_ref().and_then(|t| Some((t.query_set, t.end?)));
        let pass = handle_of(
            s.encoder
                .compute_pass(&mut s.stream, s.splits.at(encoder), &stamp),
        );
        s.splits.opened(pass, encoder, true, end);
        pass
    }

    pub fn dispatch(&self, pass: Id, x: u32, y: u32, z: u32) {
        let s = self.s();
        call::dispatch(&mut s.stream, s.splits.pass_at(pass), [x, y, z]);
        s.splits.ran(pass);
    }

    /// Workgroups as `buffer` holds them from `offset`, across, up and deep:
    /// a count only the GPU knows, written in a command buffer before this
    /// one (gpu/split.rs).
    #[track_caller]
    pub fn dispatch_indirect(&self, pass: Id, buf: Id, off: u32) {
        let at = self.indirect(pass, buf);
        let s = self.s();
        call::dispatch_indirect(&mut s.stream, at, buf, off);
        s.splits.ran(pass);
    }

    pub fn set_pipeline(&self, pass: Id, pipeline: Id) {
        let s = self.s();
        call::set_pipeline(&mut s.stream, s.splits.pass_at(pass), pipeline);
        s.splits.piped(pass, pipeline);
    }

    /// What the pass draws held to `[x, y, width, height]` of its targets.
    pub fn set_scissor(&self, pass: Id, rect: [u32; 4]) {
        call::set_scissor(&mut self.s().stream, pass, rect);
    }

    pub fn set_bind_group(&self, pass: Id, idx: u32, group: Id, offsets: &[u32]) {
        let s = self.s();
        call::set_bind_group(&mut s.stream, s.splits.pass_at(pass), idx, group, offsets);
        s.splits.bound(pass, idx, group, offsets);
    }

    /// A size of 0 is the rest of the buffer.
    pub fn set_vertex_buf(&self, pass: Id, slot: u32, buf: Id, off: u32, size: u32) {
        call::set_vertex_buffer(
            &mut self.s().stream,
            pass,
            slot,
            buf,
            off,
            (size != 0).then_some(size),
        );
    }

    /// Indices are u32.
    pub fn set_idx_buf(&self, pass: Id, buf: Id, off: u32, size: u32) {
        call::set_index_buffer(
            &mut self.s().stream,
            pass,
            buf,
            IndexFormat::uint32,
            off,
            (size != 0).then_some(size),
        );
    }

    pub fn draw(
        &self,
        pass: Id,
        vertices: u32,
        instances: u32,
        first_vertex: u32,
        first_instance: u32,
    ) {
        let s = self.s();
        call::draw(
            &mut s.stream,
            pass,
            vertices,
            instances,
            first_vertex,
            first_instance,
        );
        s.splits.ran(pass);
    }

    pub fn draw_indexed(
        &self,
        pass: Id,
        indices: u32,
        instances: u32,
        first_idx: u32,
        first_instance: u32,
    ) {
        let s = self.s();
        call::draw_indexed(
            &mut s.stream,
            pass,
            indices,
            instances,
            first_idx,
            0,
            first_instance,
        );
        s.splits.ran(pass);
    }

    #[track_caller]
    pub fn draw_indirect(&self, pass: Id, buf: Id, off: u32) {
        let at = self.indirect(pass, buf);
        let s = self.s();
        call::draw_indirect(&mut s.stream, at, buf, off);
        s.splits.ran(pass);
    }

    #[track_caller]
    pub fn draw_indexed_indirect(&self, pass: Id, buf: Id, off: u32) {
        let at = self.indirect(pass, buf);
        let s = self.s();
        call::draw_indexed_indirect(&mut s.stream, at, buf, off);
        s.splits.ran(pass);
    }

    /// Ends the pass and lets its handle go.
    pub fn end(&self, pass: Id) {
        let s = self.s();
        s.encoder.end(&mut s.stream, s.splits.pass_at(pass));
        s.splits.closed(pass);
    }

    /// Submits the encoder's command buffers, each alone and in order, and
    /// lets their handles go.
    pub fn submit(&self, encoder: Id) {
        let s = self.s();
        for e in s.splits.submitted(encoder) {
            s.encoder.submit(&mut s.stream, &[e]);
        }
    }

    /// The frame's encoder, its command buffers submitted each alone and in
    /// order, the last with its spans resolved, copied out and mapped back
    /// through a read none is out on, where it wrote any, submitted. The
    /// read asked first of those out is taken in first, where it landed, and
    /// may be asked again: its ticket is the order it was asked in, which
    /// its fence holds once it lands. Which read this frame's spans are
    /// on their way back in.
    pub fn submit_frame(&self, encoder: Id, written: u64) -> Option<usize> {
        let s = self.s();
        s.gpu_landed = take_stamps(s);
        let free = (0..READS).find(|&r| s.stamps_out[r] == 0);
        let buffers = s.splits.submitted(encoder);
        let (Some((set, resolve, mapped)), Some(r), true) = (s.stamps, free, written != 0) else {
            for e in buffers {
                s.encoder.submit(&mut s.stream, &[e]);
            }
            return None;
        };
        let bytes = 2 * SPANS as u32 * 8;
        let last = *buffers.last().or_bug(Errno::ENOTRECOVERABLE);
        call::resolve_query_set(&mut s.stream, last, set, 0, 2 * SPANS as u32, resolve, 0);
        call::copy_buffer_to_buffer(&mut s.stream, last, (resolve, 0), (mapped[r], 0), bytes);
        for e in buffers {
            s.encoder.submit(&mut s.stream, &[e]);
        }
        s.stamps_written[r] = written;
        s.stamps_asked_cnt = s.stamps_asked_cnt.wrapping_add(1).max(1);
        s.stamps_out[r] = s.stamps_asked_cnt;
        let at = s.at as usize + read_at(r) * WORD;
        // SAFETY: the head's read: its fence, its result and its ticks,
        // the host's to write.
        let (fence, ticks) = unsafe {
            (
                &*mem::at::<[u32; 2]>(at),
                core::slice::from_raw_parts_mut(mem::at::<u8>(at + 2 * WORD), 2 * SPANS * 8),
            )
        };
        call::map_async(&mut s.stream, mapped[r], 0, ticks, fence, s.stamps_out[r]);
        s.stamps_asked = Some(r);
        Some(r)
    }

    /// Configured in the format the device prefers, opaque.
    pub fn configure(&self, surface: u32) {
        call::configure(
            &mut self.s().stream,
            handle_slot(surface),
            None,
            TEXTURE_USAGE::RENDER_ATTACHMENT,
            CanvasAlphaMode::opaque,
        );
    }

    pub fn resize(&self, surface: u32, width: u32, height: u32) {
        call::resize(&mut self.s().stream, handle_slot(surface), [width, height]);
    }

    /// A view of the surface's texture for this frame.
    pub fn cur(&self, surface: u32) -> Id {
        let s = self.s();
        handle_of(s.encoder.current(&mut s.stream, handle_slot(surface)))
    }

    /// The spans' query set, two queries a span, the buffer they resolve into
    /// and those mapped back, a read's each.
    pub fn spans(&self) -> Id {
        let s = self.s();
        let set = handle_of(s.encoder.query_set(
            &mut s.stream,
            QueryType::timestamp,
            2 * SPANS as u32,
        ));
        let resolve = handle_of(s.encoder.buffer(
            &mut s.stream,
            &encode::BufferDescriptor {
                size: (2 * SPANS * 8) as u32,
                usage: BUFFER_USAGE::QUERY_RESOLVE | BUFFER_USAGE::COPY_SRC,
                mapped_at_creation: false,
            },
        ));
        let mapped = core::array::from_fn(|_| {
            handle_of(s.encoder.buffer(
                &mut s.stream,
                &encode::BufferDescriptor {
                    size: (2 * SPANS * 8) as u32,
                    usage: BUFFER_USAGE::MAP_READ | BUFFER_USAGE::COPY_DST,
                    mapped_at_creation: false,
                },
            ))
        });
        s.stamps = Some((set, resolve, mapped));
        set
    }

    /// The objects made and not let go, by the op that made them (xpute-gpu
    /// encode `Gpu::objects`).
    pub fn objects(&self) -> &'static [u32; OBJECT_OPS] {
        self.s().encoder.objects()
    }

    /// Span `span`'s time in ms from the last read taken in.
    pub fn gpu_ms(&self, span: u32) -> f64 {
        self.s().gpu_ms[span as usize]
    }

    /// The time the last read's spans ran, ms, those that overlap counted once.
    pub fn gpu_busy_ms(&self) -> f64 {
        self.s().gpu_busy_ms
    }

    /// The read the frame just submitted took in, where it took one: its
    /// figures were measured with it.
    pub fn gpu_landed(&self) -> Option<usize> {
        self.s().gpu_landed
    }

    /// `size` bytes of `from` from `at` into `to` from `to_at`, on `encoder`.
    pub fn copy_buf(&self, encoder: Id, (from, at): (Id, u32), (to, to_at): (Id, u32), size: u32) {
        let s = self.s();
        call::copy_buffer_to_buffer(
            &mut s.stream,
            s.splits.at(encoder),
            (from, at),
            (to, to_at),
            size,
        );
        s.splits.copied(encoder, to);
    }

    /// `length` bytes of `id`, a buffer made `MAP_READ | COPY_DST` and copied
    /// into on an encoder submitted before, mapped back from `offset` into
    /// the kernel's memory at `address`; `fence`, the mapping's own fence
    /// and result, holds `ticket` once they are in or refused, a turn or
    /// more later, and only then may `id` be mapped again (xpute MAP_ASYNC).
    pub fn map_read(
        &self,
        id: Id,
        off: u32,
        length: u32,
        addr: u32,
        fence: &[u32; 2],
        ticket: u32,
    ) {
        // SAFETY: the kernel's own range, which it asked to be written and
        // does not read while the mapping is out.
        let into = unsafe {
            core::slice::from_raw_parts_mut(mem::at::<u8>(addr as usize), length as usize)
        };
        call::map_async(&mut self.s().stream, id, off, into, fence, ticket);
    }
}

#[cfg(test)]
#[path = "api.test.rs"]
mod test;
