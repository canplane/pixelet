// pxl-render/gpu/api.rs

//! The WebGPU calls the renderer makes. They are written through xpute's encoder (xpute-gpu
//! `encode`) into one stream in the core's memory, and the runtime runs the stream after the
//! turn. The page's runtime runs it on WebGPU and the device app's on wgpu (xpute-gpu
//! `runtime`), from the same records. What is pixelet's own stays here: the pipeline records
//! expanded into descriptors, the shape of a pass, and the timing of the GPU spans a frame is
//! timed in (gpu/span.spec.rs).
//!
//! The stream's head holds what the runtime writes back for the core: the device's block (its
//! limits, features, timestamp period, preferred surface format and status) and the spans'
//! times.

use xpute_core::abi::handle::handle_slot;
use xpute_core::global::Global;
use xpute_core::ipc::stream::{Stream, WORD};
use xpute_core::mem;
use xpute_gpu::device;
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
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

pub use xpute_gpu::encode::load_acquire;
pub use xpute_gpu::encode::signaled;
pub use xpute_gpu::enums::TextureFormat as Format;
pub use xpute_gpu::spec::RESULT;

use crate::api_spec::{record, stream};
use crate::frame::shader::access::{self, END};
use crate::frame::shader::floor::{self, BIND_GROUPS, VERTEX_BUFFERS};
use crate::frame::shader::group::BUFFERS;
use crate::frame::shader::record::{FRAGMENT_ENTRY, VERTEX_ENTRY};
use crate::gpu::span_spec::{GpuSpanName, GPU_SPAN_NAMES};
use crate::gpu::split::Split;

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

/// The most objects the renderer holds at once: the size of the encoder's handle table.
pub const OBJECTS: usize = 1 << 15;

use crate::frame::shader::record::GROUP_ENTRIES;

const SPANS: usize = GPU_SPAN_NAMES;

/// WebGPU's default `minUniformBufferOffsetAlignment`. Every adapter's limit is a power of two
/// no larger than this, so a larger value comes from no real device and is refused (`device`).
pub const UNIFORM_ALIGN_MAX: u32 = 256;

/// An image file as its asker hands it: a name and the file's bytes. The asker chooses the
/// name and gives one file the same name wherever it hands it. The asker hands the bytes until
/// the texture lands (`take_complete`) and an empty slice after that, while the texture
/// belongs to whoever took it (`Gpu::image`).
pub type Image = (u32, &'static [u8]);

/// The head's words that follow the stream's own. The device's block comes first. Then come
/// `READS` reads of the spans, each a fence, a result, and the begin and end ticks of every
/// span of the frame it timed, two words a tick (`read_at`). The stream records the head's
/// length itself, so the runtime does not read this layout.
const DEVICE_AT: usize = Stream::OWN_WORDS;
const READS_AT: usize = DEVICE_AT + device::WORDS;
/// How many reads of the spans may be out at once, each into its own mapped buffer. A frame's
/// read lands a frame or two after the frame, so with three no new read waits for an earlier
/// one to be taken.
pub const READS: usize = 3;
const READ_WORDS: usize = 2 + 2 * 2 * SPANS;
const HEADER: usize = READS_AT + READS * READ_WORDS;

/// The head word that holds read `k`'s fence. The read's result and ticks follow it.
const fn read_at(k: usize) -> usize {
    READS_AT + k * READ_WORDS
}

/// The device as its block describes it, once the runtime has written the block.
#[derive(Clone, Copy)]
pub struct Device {
    pub align: u32,
    pub timestamps: bool,
    /// The format the device's surfaces show best. The canvas's pipelines target it.
    pub canvas: Format,
    period_ns: f32,
}

/// A span's pair of timestamp queries. The span's first pass writes the begin and every pass
/// writes the end, so the last pass's end stands. A span with no passes reads 0.
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

/// The place in `texture` that a copy reads or writes: `origin` (x, y, layer) in its first mip.
pub fn place(texture: Id, origin: [u32; 3]) -> encode::TexelCopyTexture {
    encode::TexelCopyTexture {
        texture,
        mip: 0,
        origin,
        aspect: TextureAspect::all,
    }
}

/// An encoder being recorded, with the timestamp writes of the frame's spans. Each pass is
/// handed this and writes the timestamps of its own span.
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

/// What a render pass has set: its pipeline, bind groups, vertex lanes and index buffer. A
/// group set with dynamic offsets is held as unset, because its offsets change from draw to
/// draw.
#[derive(Default)]
pub struct DrawState {
    pipeline: Id,
    groups: [Id; BIND_GROUPS],
    lanes: [(Id, u32, u32); VERTEX_BUFFERS],
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

/// A render pass handed from draw to draw. It skips setting a pipeline, bind group, vertex lane
/// or index buffer that its `DrawState` says the pass already has. Every call on the pass goes
/// through it, so what the state holds is what the pass has.
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

    /// Limits what the pass draws to `rect`, `[x, y, width, height]` in its targets' pixels.
    pub fn set_scissor(&self, rect: [u32; 4]) {
        self.g.set_scissor(self.pass, rect);
    }

    /// Sets the stencil reference that the pass's draws test against and write (`Stencil`).
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

/// The depth format of the targets a pass can draw into through a gate: the world surfaces'
/// depth and the light map's. A frame that is not drawn whole is drawn through a gate, which
/// marks the tiles the frame claims. The format is a 32-bit float with a stencil, and the
/// stencil holds those marks, so the passes after the gate skip the other tiles (`Stencil`).
pub const MASKED_DEPTH: Format = Format::depth32float_stencil8;

/// The format of the world's color as its passes leave it: toned, linear, and blended in place.
/// The color is gathered over frames (gpu/resolve.rs) and encoded onto the canvas once
/// (gpu/finish.rs). It is a float, so the color has no levels of its own, and the finish's
/// dither works only between the levels the canvas has.
pub const WORLD_COLOR: Format = Format::rgba16float;

/// The most color targets a pipeline draws into. The finish draws into two: the canvas and the
/// picture that the world's surface keeps (gpu/finish.rs).
pub const TARGETS: usize = 2;

/// What a pipeline does with the stencil beside its depth (`MASKED_DEPTH`). In each tile, the
/// stencil holds the number of the gated frame that last claimed the tile. `Unused` leaves the
/// stencil alone. `Masked` passes only where the stencil holds the pass's reference, which is
/// this frame's number; every draw of the world's pass is masked. `Mask` writes the reference,
/// as the gate does (gpu/renderer.rs `frame`). A pass clears the stencil to 0 unless it keeps
/// it (`PassDesc::load_stencil`), and WebGPU starts every pass with a reference of 0, so a pass
/// with no gate passes everywhere.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Stencil {
    Unused,
    Masked,
    Mask,
}

/// How a pass starts an attachment: cleared, loaded, or, for a depth, loaded and read only.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Load {
    Clear,
    Load,
    ReadOnly,
}

/// A render pass's targets and how each starts. The color is cleared to transparent, cleared
/// to what `color_clear` gives, or loaded. A second color (`second_color`) starts as the first
/// does. The depth is cleared to `depth_clear`, loaded, or read only. The depth's stencil,
/// where it has one, is cleared to 0 or loaded (`load_stencil`). A target's handle is 0 where
/// the pass has none.
pub struct PassDesc {
    color: Id,
    second: Id,
    color_load: Load,
    color_clear: [f32; 4],
    depth: Id,
    depth_load: Load,
    depth_clear: f32,
    stencil_load: Load,
    stamp: Option<encode::TimestampWrites>,
}

impl PassDesc {
    /// Loads the stencil as the last pass left it, holding the number of the gated frame that
    /// last claimed each tile (`Stencil`).
    pub fn load_stencil(self) -> PassDesc {
        PassDesc {
            stencil_load: Load::Load,
            ..self
        }
    }

    /// Clears the color to `rgba`, given in linear values.
    pub fn color_clear(self, rgba: [f32; 4]) -> PassDesc {
        PassDesc {
            color_clear: rgba,
            ..self
        }
    }

    /// Adds `view` as the second color target, at location 1, started as the first is. A
    /// pipeline built with two targets draws into it (`PipelineCache::build_targets`).
    pub fn second_color(self, view: Id) -> PassDesc {
        PassDesc {
            second: view,
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
        second: 0,
        color_load,
        color_clear: [0.0; 4],
        depth,
        depth_load,
        depth_clear,
        stencil_load: Load::Clear,
        stamp,
    }
}

/// A bind group layout, with the group words it was made from: a (binding, stages, kind)
/// triple per binding, as a pipeline record holds them (frame/shader/record.rs) or as a
/// compute group gives them. Only `Gpu::layout` makes one, so every bind group over it can be
/// checked against those words.
#[derive(Clone, Copy, Default)]
pub struct Layout {
    id: Id,
    group: &'static [u32],
}

impl Layout {
    pub fn binds(&self, binding: u32) -> bool {
        self.group
            .as_chunks::<3>()
            .0
            .iter()
            .any(|w| w[0] == binding)
    }

    /// The buffers among `entries` that the group binds as writable storage, each with its
    /// binding.
    fn writable<'e>(&self, entries: &'e [Entry]) -> impl Iterator<Item = (u32, Id)> + 'e {
        let words = self.group.as_chunks::<3>().0;
        entries.iter().filter_map(move |e| match e.resource {
            encode::Resource::Buffer { buffer, .. }
                if words
                    .iter()
                    .any(|w| w[0] == e.binding && w[2] == record::STORAGE_RW) =>
            {
                Some((e.binding, buffer))
            }
            _ => None,
        })
    }

    /// Checks that `entries` name each of the group's bindings once, each with a resource of
    /// the binding's kind: a buffer for a buffer kind, a sampler for a sampler kind, and a view
    /// for the rest. Anything else is a bug.
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

pub use xpute_gpu::encode::BindGroupEntry as Entry;

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

/// Whether two entries bind the same resource at the same binding. Comparing handles is
/// enough: a handle is generational (xpute_core abi/handle.rs), so a resource let go and made
/// again gets a new handle, never its old one.
fn same_entry(a: &Entry, b: &Entry) -> bool {
    use encode::Resource::{Buffer, Sampler, View};
    a.binding == b.binding
        && match (a.resource, b.resource) {
            (
                Buffer {
                    buffer: x,
                    offset: o,
                    size: n,
                },
                Buffer {
                    buffer: y,
                    offset: p,
                    size: m,
                },
            ) => x == y && o == p && n == m,
            (Sampler(x), Sampler(y)) | (View(x), View(y)) => x == y,
            _ => false,
        }
}

/// A bind group kept across frames over the resources its entries name. It is made again when
/// one of those resources is made again (`same_entry`). Its owner sets it in each frame it
/// draws, before the frame's commands begin (`frame_begin`), because a frame lets go within
/// itself of every object it makes. The owner lets the group go when the owner itself goes.
/// The group that a new one replaces names a resource that was let go and that the runtime
/// has destroyed, and the old group is let go at that point.
#[derive(Default)]
pub struct Group {
    layout: Id,
    entries: Vec<Entry>,
    id: Id,
}

impl Group {
    /// The group over `layout` and `entries`: the kept one where it was made over both, and
    /// otherwise a new one made in its place.
    #[track_caller]
    pub fn set(&mut self, g: &Gpu, layout: Layout, entries: &[Entry]) -> Id {
        let same = self.id != 0
            && self.layout == layout.id
            && self.entries.len() == entries.len()
            && self
                .entries
                .iter()
                .zip(entries)
                .all(|(a, b)| same_entry(a, b));
        if !same {
            release(self.id);
            self.id = g.bind_group(layout, entries);
            self.layout = layout.id;
            self.entries.clear();
            self.entries.extend_from_slice(entries);
        }
        self.id
    }

    /// The group as last set, or 0 before the first set.
    pub fn id(&self) -> Id {
        self.id
    }

    pub fn release(&mut self) {
        release(self.id);
        *self = Group::default();
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
    /// The encoder lies at the range's end, past the stream. It is never built or moved:
    /// all-zero bytes are an encoder that has made nothing (xpute-gpu encode).
    encoder: &'static mut Encoder<OBJECTS>,
    /// Where the stream's words begin in the core's memory.
    at: u32,
    /// Whether a frame's transaction is open. Inside one, a record that does not fit drops the
    /// frame; outside one, it is a bug.
    in_frame: bool,
    /// Whether the device's block has been asked for. It is read once its status is READY.
    device_out: bool,
    device: Option<Device>,
    /// The spans' query set, the buffer it resolves into, and one buffer per read that the
    /// resolved ticks are copied into and mapped back through.
    stamps: Option<(Id, Id, [Id; READS])>,
    /// For each read, the spans it timed, one bit a span.
    stamps_written: [u64; READS],
    /// For each read, the order in which it was asked, or 0 where it is not out. The reads are
    /// taken in that order.
    stamps_out: [u32; READS],
    stamps_asked_cnt: u32,
    /// The read this frame asked for. Its mapping's ticket is written as the frame is encoded,
    /// so a frame dropped whole must take the read back (`frame_commit`), or the read would
    /// stay out and never land.
    stamps_asked: Option<usize>,
    /// Each span's own time from the last read taken in, in ms.
    gpu_ms: [f64; SPANS],
    /// The time the last read's spans ran, in ms, with overlapping spans counted once. This is
    /// the frame's GPU time; the spans' sum is not, where passes overlap. A pass with no work
    /// may be dropped and leave its begin tick unwritten; its span is then counted from the
    /// latest end before it, so it adds its own length and not the time since an older tick.
    gpu_busy_ms: f64,
    /// The read that the last submitted frame took in, if it took one. When it did, the span
    /// times are that read's; when it did not, they are still the earlier read's, not
    /// measured again.
    stamps_complete: Option<usize>,
    /// The images on their way into textures (`Gpu::image`).
    images: Vec<ImageCopy>,
    /// The images whose textures were taken on this device since `take_complete` last ran.
    complete: Vec<u32>,
    /// How many devices have been lost so far. Whatever was handed to the GPU under an earlier
    /// count is on no device that exists now.
    generation: u32,
    /// The objects let go while the runtime held the stream. They are destroyed once it hands
    /// the stream back (`writable`).
    frees: Vec<Id>,
    /// Whether a device answered while the runtime held the stream. Its block is then asked
    /// for once the runtime hands the stream back.
    device_pending: bool,
    /// Whether the device's block said LOST when last read. Only the runtime writes the block,
    /// so a loss is reported once by comparing against this flag, not by clearing the block.
    lost: bool,
    /// The memory ranges that WRITE_BUFFER_AT records name, as address and length. `ranges`
    /// holds those recorded since the last publish. `ranges_out` holds those of the hand-off
    /// the runtime holds until it gives the stream back (`in_flight`).
    ranges: Vec<(usize, usize)>,
    ranges_out: Vec<(usize, usize)>,
    /// Where each encoder is cut into command buffers (gpu/split.rs).
    splits: Split,
}

/// An image file being copied into its texture. The asker keeps the file's bytes until the
/// texture's fence says the copy landed or was refused. Each copy is asked once, so its ticket
/// is the first of its fence's sequence (`IMAGE_TICKET`).
struct ImageCopy {
    image: u32,
    texture: Id,
    fence: Box<[u32; 2]>,
    /// Whether `images_complete` has reported its landing.
    told: bool,
}

const IMAGE_TICKET: u32 = 1;

static STATE: Global<Option<State>> = Global::new();

pub struct Gpu(());

static GPU: Gpu = Gpu(());

fn state() -> &'static mut State {
    STATE.get(|| None).as_mut().or_bug(Errno::ENODEV)
}

fn header(k: usize) -> &'static mut u32 {
    let at = state().at as usize + k * WORD;
    // SAFETY: a word of the stream's head, the core's and the runtime's alone.
    unsafe { &mut *mem::at::<u32>(at) }
}

/// Read `r`'s fence and result, which only the runtime writes.
fn read_fence(r: usize) -> &'static [u32; 2] {
    let at = state().at as usize + read_at(r) * WORD;
    // SAFETY: two words of the stream's head, which the core only reads.
    unsafe { &*mem::at::<[u32; 2]>(at) }
}

/// The bytes the encoder takes at the end of the installed range.
const ENCODER_BYTES: usize = size_of::<Encoder<OBJECTS>>().next_multiple_of(8);
const _: () =
    assert!(align_of::<Encoder<OBJECTS>>() <= 8 && ENCODER_BYTES < stream::RESERVE as usize);

/// # Safety
/// `at` must address `bytes` zeroed bytes, 8-aligned, that nothing else writes for as long as
/// the module runs. The stream takes the start of the range and the encoder's handle table
/// takes its end.
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
            Stream::new(mem::core(), at as usize, words, HEADER),
            &mut *mem::at::<Encoder<OBJECTS>>(at as usize + bytes as usize - ENCODER_BYTES),
        )
    };
    STATE.set(Some(State::over(stream, encoder, at)));
    at
}

/// Installs the calls' state for a test: a stream in a region of its own, apart from the core's
/// memory, with its encoder beside it, and `device` as though its block had been written. No
/// runtime runs this stream.
#[cfg(test)]
pub(crate) fn install_apart(device: Device) {
    const WORDS: usize = 1 << 25;
    let region: &'static mut [u64] = Vec::leak(vec![0; WORDS / 2]);
    // SAFETY: leaked, so it lives as long as the process, and nothing else names it.
    let stream = unsafe {
        let mem = mem::Memory::new(region.as_mut_ptr().expose_provenance(), WORDS * WORD);
        Stream::new(mem, 0, WORDS, HEADER)
    };
    // SAFETY: all-zero bytes are an encoder that has made nothing (xpute-gpu encode).
    let encoder = unsafe { Box::<Encoder<OBJECTS>>::new_zeroed().assume_init() };
    let mut s = State::over(stream, Box::leak(encoder), 0);
    s.device = Some(device);
    STATE.set(Some(s));
}

/// A made-up device for `install_apart`.
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
            stamps_complete: None,
            images: Vec::new(),
            complete: Vec::new(),
            generation: 0,
            frees: Vec::new(),
            device_pending: false,
            lost: false,
            ranges: Vec::new(),
            ranges_out: Vec::new(),
            splits: Split::default(),
        }
    }
}

/// Where the stream lies in the core's memory once it is installed. The runtime runs the
/// stream there after each turn.
pub fn stream_at() -> Option<u32> {
    STATE.get(|| None).as_ref().map(|s| s.at)
}

/// The renderer's calls, once the device's block is in and while the stream is the core's to
/// write (`stream_free`).
pub fn gpu() -> Option<&'static Gpu> {
    device().filter(|_| stream_free()).map(|_| &GPU)
}

/// Whether the runtime has handed the stream back since the core last published it
/// (xpute_core ipc/stream.rs R3). Until it has, the core records nothing and the frame waits.
pub fn stream_free() -> bool {
    STATE.get(|| None).as_mut().is_none_or(writable)
}

/// `stream_free`, and once the stream is free, records what waited for it: it asks for the
/// device's block and destroys the objects let go meanwhile.
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

/// The device as its block describes it, once the runtime has written the block. The block
/// is asked for when a device answers (`on_device_answered`), and the first call after its
/// status turns READY reads it.
pub fn device() -> Option<Device> {
    let s = STATE.get(|| None).as_mut()?;
    if s.device.is_none()
        && s.device_out
        && load_acquire(header(DEVICE_AT + device::STATUS_AT)) == STATUS::READY
    {
        let word = |k: usize| *header(DEVICE_AT + k);
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
            word(device::FEATURES_AT + k / 32) >> (k % 32) & 1 != 0
        };
        let canvas = *Format::ALL
            .get(word(device::FORMAT_AT) as usize)
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
            period_ns: f32::from_bits(word(device::PERIOD_AT)),
        });
    }
    s.device
}

/// Asks for the block of a device that answered, as soon as the stream is the core's to
/// write. `device` reads the block once the runtime has written it.
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
    // SAFETY: the head's block, the runtime's to write.
    let block = unsafe { core::slice::from_raw_parts_mut(mem::at::<u32>(at), device::WORDS) };
    call::device(&mut s.stream, block);
    s.device_out = true;
}

/// Whether the device was lost since this was last asked, as the runtime reports in the
/// device's block (xpute STATUS.LOST). Each loss is reported once, and the block is left as
/// the runtime wrote it.
pub fn take_device_lost() -> bool {
    let Some(s) = STATE.get(|| None).as_mut() else {
        return false;
    };
    let lost = load_acquire(header(DEVICE_AT + device::STATUS_AT)) == STATUS::LOST;
    !core::mem::replace(&mut s.lost, lost) && lost
}

/// How many devices have been lost so far (`on_device_lost`). Whoever hands the GPU anything
/// notes the count it was handed under; what was handed under another count is gone.
pub fn device_generation() -> u32 {
    STATE.get(|| None).as_ref().map_or(0, |s| s.generation)
}

/// Takes in the ticks of the earliest-asked read that is out, if its fence says they landed.
/// It sets each span's own time and the busy time, frees the read for another frame to ask,
/// and returns which read it took. A read that was refused is freed and measures nothing. One
/// read is taken a frame, as one is asked a frame.
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
        |k: usize| u64::from(*header(ticks + 2 * k)) | u64::from(*header(ticks + 2 * k + 1)) << 32;
    let mut ran = [(0u64, 0u64); SPANS];
    for (k, r) in ran.iter_mut().enumerate() {
        let (b, e) = (tick(2 * k), tick(2 * k + 1));
        if written & 1u64 << k != 0 && e >= b {
            *r = (b, e);
        }
    }
    // A span's own time is its end less the later of its begin and the latest end before it.
    // A tiled GPU starts a render pass's vertex work early, so the pass's begin is stamped
    // before the passes it waits on have finished. A render pass's end and a compute pass's
    // begin are stamped in order, so counting from the latest earlier end leaves out the time
    // that belongs to the passes before it.
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

/// The device was lost with every object on it, so the handles start over.
pub fn on_device_lost() {
    if let Some(s) = STATE.get(|| None).as_mut() {
        // SAFETY: the encoder's own bytes; zeroed, it has made nothing.
        unsafe { core::ptr::write_bytes(core::ptr::from_mut(s.encoder), 0, 1) };
        (s.device_out, s.device, s.stamps) = (false, None, None);
        s.images.clear();
        s.complete.clear();
        s.generation = s.generation.wrapping_add(1);
        s.frees.clear();
        s.device_pending = false;
        s.stamps_out = [0; READS];
    }
}

/// Hands the turn's records to the runtime, and with them the ranges they name
/// (`in_flight`).
pub fn gpu_publish() {
    if let Some(s) = STATE.get(|| None).as_mut() {
        if s.stream.is_free() {
            core::mem::swap(&mut s.ranges_out, &mut s.ranges);
            s.ranges.clear();
        }
        s.stream.publish();
    }
}

/// Whether `vals` overlaps memory that a WRITE_BUFFER_AT record names, among the records not
/// yet published or in the hand-off the runtime has not given back (xpute_core ipc/stream.rs
/// R3). While it does, the runtime is still to read that memory, so the values' writer leaves
/// them unchanged and in place.
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

/// Calls `complete` with each image whose texture was taken on the current device since this
/// last ran. The image's asker may then let its file go, and hands the file again only after
/// the device is lost.
pub fn take_complete(mut complete: impl FnMut(u32)) {
    if let Some(s) = STATE.get(|| None).as_mut() {
        s.complete.drain(..).for_each(&mut complete);
    }
}

/// Whether an image handed to the device (`Gpu::image`) landed since this last ran. Its
/// fence's store woke the turn, and a frame must run to take the texture.
pub fn images_complete() -> bool {
    STATE.get(|| None).as_mut().is_some_and(|s| {
        s.images.iter_mut().fold(false, |any, a| {
            let complete = !a.told && signaled(&a.fence[0], IMAGE_TICKET);
            a.told |= complete;
            any | complete
        })
    })
}

/// Opens a frame's transaction. The frame's records stand or fall together: if they do not
/// fit the stream, they are dropped whole, with the objects they made.
pub fn frame_begin() {
    let s = state();
    s.encoder.begin(&mut s.stream);
    s.in_frame = true;
    s.stamps_asked = None;
}

/// Closes the frame's transaction. False where the frame did not fit and nothing of it was
/// recorded; the read the frame asked for was then never mapped, so it is freed for the next
/// frame to ask.
pub fn frame_commit() -> bool {
    let s = state();
    s.in_frame = false;
    let committed = s.encoder.commit(&mut s.stream);
    if let Some(r) = s.stamps_asked.take().filter(|_| !committed) {
        s.stamps_out[r] = 0;
    }
    committed
}

use pxl_spec::asset;
/// The stream bytes a write takes, as its writer measures them. A caller sums them over the
/// writes it makes together and asks `room` before making them.
pub use xpute_gpu::encode::call::{write_buffer_bytes, write_texture_bytes};

/// The stream bytes a shader module of `code` takes (`Gpu::shader`). Its maker asks `room`
/// for them before making it.
pub fn shader_bytes(code: &str) -> usize {
    Encoder::<OBJECTS>::shader_module_bytes(code)
}

/// Whether records of `bytes` in all (`write_buffer_bytes`, `write_texture_bytes`,
/// `shader_bytes`) fit the stream while leaving `stream::RESERVE` for records that cannot
/// wait.
pub fn room(bytes: usize) -> bool {
    STATE
        .get(|| None)
        .as_ref()
        .is_some_and(|s| s.stream.fits(bytes, stream::RESERVE as usize))
}

/// An object's handle. Inside a frame, the encoder makes none where the frame no longer fits,
/// and the handle is 0: the object is dropped with the frame. Outside a frame, making none is
/// a bug.
fn handle_of(id: Option<Id>) -> Id {
    match id {
        Some(id) => id,
        None => {
            xpute_kit::ensure!(state().in_frame, ENOSPC, OBJECTS);
            0
        }
    }
}

/// Lets an object go. The runtime destroys it, and its handle is free to name another. An `id`
/// of 0 names nothing.
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

fn stencil_face(stencil: Stencil) -> encode::StencilFaceState {
    match stencil {
        Stencil::Unused => encode::STENCIL_UNUSED,
        Stencil::Masked => encode::StencilFaceState {
            compare: CompareFunction::equal,
            ..encode::STENCIL_UNUSED
        },
        Stencil::Mask => encode::StencilFaceState {
            pass: StencilOperation::replace,
            ..encode::STENCIL_UNUSED
        },
    }
}

impl Gpu {
    fn s(&self) -> &'static mut State {
        state()
    }

    /// A buffer of `size` bytes. The size must be one that every device makes and, for a
    /// storage buffer, binds (frame/shader/floor.rs `fits`). A larger size is a bug in the
    /// structure that asked for it, not a device falling short.
    #[track_caller]
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
        let at = core::panic::Location::caller();
        s.splits
            .made(buf, usage & buffer_usage::INDIRECT != 0, at.line());
        self.label(buf, "buffer", core::panic::Location::caller());
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

    /// The whole view of a texture that `volume` made.
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

    /// With `array`, the view sees the texture's layers as one array.
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

    /// A view of only the depth of a depth target that carries a stencil, for a binding to
    /// read. WebGPU binds no view of both aspects.
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

    /// A group's layout from its words, a (binding, stages, kind) triple per binding, as a
    /// pipeline record holds them.
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

    /// A render pipeline from its record (frame/shader/record.rs) over the group layouts. It
    /// draws into `targets` color targets of `format`, at locations from 0, and into a depth
    /// of `depth`, with `stencil` saying what it does with the stencil beside the depth. Where
    /// `format` is `depth32float`, it draws into the depth alone and its fragment writes no
    /// color, as the shadow map's does (gpu/shadow.rs). WebGPU makes it asynchronously: the
    /// runtime stores `ticket` at `fence` once the pipeline is made, and nothing names the
    /// pipeline before then (gpu/pipeline.rs `PipelineCache::made`).
    pub fn pipeline(
        &self,
        words: &[u32],
        layouts: &[Layout],
        module: Id,
        (format, targets): (Format, usize),
        (depth, stencil): (Format, Stencil),
        (fence, ticket): (&u32, u32),
    ) -> Id {
        let mut at = record::HEADER as usize;
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
        let blend = || match words[8] {
            record::ALPHA => Some(ALPHA_BLEND),
            record::ADD => Some(ADD_BLEND),
            _ => None,
        };
        let colors: [encode::ColorTargetState; TARGETS] =
            core::array::from_fn(|_| encode::ColorTargetState {
                format,
                blend: blend(),
                write_mask: COLOR_WRITE::ALL,
            });
        xpute_kit::ensure!(targets <= TARGETS, EINVAL, targets);
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
                    entry: VERTEX_ENTRY[words[record::HEADER as usize - 1] as usize],
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
                        &colors[..targets]
                    },
                }),
            },
            fence,
            ticket,
        ));
        s.encoder.destroy(&mut s.stream, layout);
        p
    }

    /// A compute pipeline of `module`'s entry `entry` over `layouts`. It is made
    /// asynchronously, and its fence and ticket work as `pipeline`'s do.
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

    /// A bind group over `layout`. `entries` must name each of the group's bindings once, each
    /// with a resource of the binding's kind; anything else is a bug (`Layout::holds`).
    #[track_caller]
    pub fn bind_group(&self, layout: Layout, entries: &[Entry]) -> Id {
        layout.holds(entries);
        let s = self.s();
        let group = handle_of(s.encoder.bind_group(&mut s.stream, layout.id, entries));
        s.splits.grouped(group, layout.writable(entries));
        group
    }

    /// The bind group over `layout` that `table` describes (frame/shader/group.rs). Each
    /// binding the table names is bound to its range of the frame's buffer that `buf` gives.
    /// The entries of `rest` that the layout binds and the table does not name follow. Only the
    /// table binds the frame's buffers, so an entry of `rest` that binds one is a bug. `buf`
    /// answers 0 for a buffer its caller does not hold.
    #[track_caller]
    pub fn frame_group(
        &self,
        layout: Layout,
        table: &access::Group,
        buf: impl Fn(u32) -> Id,
        rest: &[Entry],
    ) -> Id {
        let named = |binding: u32| table.binds.iter().any(|b| b.binding == binding);
        let mut entries: Vec<Entry> = table
            .binds
            .iter()
            .map(|b| match (b.at, b.words) {
                (0, END) => buf_entry(b.binding, buf(b.buf)),
                _ => buf_range_entry(
                    b.binding,
                    buf(b.buf),
                    word_bytes(b.at as usize),
                    word_bytes(b.words as usize),
                ),
            })
            .collect();
        for e in rest
            .iter()
            .filter(|e| layout.binds(e.binding) && !named(e.binding))
        {
            if let encode::Resource::Buffer { buffer, .. } = e.resource {
                let shared = (0..BUFFERS.len() as u32).any(|k| buf(k) == buffer);
                xpute_kit::ensure!(buffer == 0 || !shared, ENOTRECOVERABLE, e.binding, buffer);
            }
            entries.push(*e);
        }
        self.bind_group(layout, &entries)
    }

    pub fn canvas(&self) -> Format {
        self.s().device.or_bug(Errno::ENODEV).canvas
    }

    /// Copies `bytes` into the stream, so their owner may rewrite them at once.
    pub fn write_buf(&self, id: Id, off: u32, bytes: &[u8]) {
        call::write_buffer(&mut self.s().stream, id, off, bytes);
    }

    /// Writes `bytes` into buffer `id` in one record however large. The runtime reads the
    /// bytes where they lie when it runs the turn. They are the runtime's from the turn's
    /// publish until it hands the stream back (xpute_core ipc/stream.rs R3), so their owner
    /// asks `in_flight` before it writes them again.
    pub fn upload_buf(&self, id: Id, off: u32, bytes: &[u8]) {
        let s = self.s();
        if call::write_buffer_at(&mut s.stream, id, off, bytes) {
            s.ranges.push((bytes.as_ptr() as usize, bytes.len()));
        }
    }

    /// Writes the whole first mip of `layer`, four bytes a texel.
    pub fn write_texture(&self, id: Id, layer: u32, width: u32, height: u32, bytes: &[u8]) {
        call::write_texture(
            &mut self.s().stream,
            &place(id, [0, 0, layer]),
            [width * 4, height],
            [width, height, 1],
            bytes,
        );
    }

    /// Decodes the image file `file` (PNG, JPEG or WebP) into `texture`. The runtime stores
    /// `ticket` at `fence` once the image is there or was refused.
    pub fn copy_image(&self, texture: Id, file: &[u8], fence: &[u32; 2], ticket: u32) {
        let to = place(texture, [0, 0, 0]);
        call::copy_external_image(&mut self.s().stream, &to, false, false, file, fence, ticket);
    }

    /// The texture that the image file `file` is copied into, which its asker names `image`.
    /// Each image is copied once on this device, and the texture goes to whoever takes it once
    /// it has landed. The first ask hands the file to the device, and the answer is None while
    /// the image lands. The answer stays None for good for a file whose header gives no size
    /// (pxl-spec `asset::image_size`), which is never handed to the device; for a file the
    /// runtime could not decode; and for the empty file of an image already taken on this
    /// device. The asker keeps the file's bytes until the texture is taken (`take_complete`),
    /// because the runtime copies them out only as it runs the turn.
    pub fn image(&self, image: u32, file: &'static [u8]) -> Option<Id> {
        let s = self.s();
        if let Some(k) = s.images.iter().position(|a| a.image == image) {
            let fence = &s.images[k].fence;
            if !signaled(&fence[0], IMAGE_TICKET) || fence[1] != RESULT::OK {
                return None;
            }
            s.complete.push(image);
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
        self.s().images.push(ImageCopy {
            image,
            texture,
            fence,
            told: false,
        });
        None
    }

    /// Labels `id` with `kind` and the source line that made it, so that a validation error
    /// names a buffer, encoder or pass by where it was made.
    fn label(&self, id: Id, kind: &str, at: &core::panic::Location) {
        let mut name = Label::default();
        let file = at.file().rsplit('/').next().unwrap_or(at.file());
        let _ = core::fmt::write(&mut name, format_args!("{kind} {file}:{}", at.line()));
        let s = self.s();
        call::label(&mut s.stream, id, name.text());
    }

    /// A command encoder. It is cut into several command buffers where its passes read args
    /// written earlier in it (gpu/split.rs), and it is submitted as those command buffers.
    #[track_caller]
    pub fn encoder(&self) -> Id {
        let s = self.s();
        let encoder = handle_of(s.encoder.encoder(&mut s.stream));
        s.splits.begun(encoder);
        self.label(encoder, "encoder", core::panic::Location::caller());
        encoder
    }

    /// Makes `encoder` record into a new command buffer from here on (gpu/split.rs).
    #[track_caller]
    fn split(&self, encoder: Id) {
        let s = self.s();
        let next = handle_of(s.encoder.encoder(&mut s.stream));
        s.splits.split(encoder, next);
        self.label(next, "encoder split", core::panic::Location::caller());
    }

    /// The pass in which an indirect call on `pass` that reads `buf` is recorded. Where `buf`
    /// was written in the pass's own command buffer (`Split::cuts`), a compute pass is ended
    /// and begun again in a new command buffer, with its pipeline and groups set again and its
    /// span's end moved to the new part. A render pass already begins in a command buffer of
    /// its own where args were written before it (`Split::draws_apart`), so only its own draws
    /// could have written `buf`, which is a bug.
    #[track_caller]
    fn indirect(&self, pass: Id, buf: Id) -> Id {
        let s = self.s();
        // An indirect call must not write the buffer it reads its count from. A buffer bound
        // writable in a group set for the call, or in a render pass in any group set in the
        // pass, is refused at the call's line. The bug's first word names the group (its
        // index, or in a render pass its handle) and the binding; its second names the line
        // that made the buffer and the buffer. Each pair is a word's high and low halves.
        if let Some((idx, binding)) = s.splits.binds_writable(pass, buf) {
            xpute_kit::bug!(
                ENOTRECOVERABLE,
                u64::from(idx) << 32 | u64::from(binding),
                u64::from(s.splits.made_at(buf)) << 32 | u64::from(buf)
            );
        }
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
        self.label(at, "compute indirect", core::panic::Location::caller());
        let s = self.s();
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

    #[track_caller]
    pub fn pass(&self, encoder: Id, d: &PassDesc) -> Id {
        let named = encoder;
        if self.s().splits.draws_apart(named) {
            self.split(named);
        }
        let encoder = self.s().splits.at(named);
        let colors = [d.color, d.second].map(|view| encode::ColorAttachment {
            view,
            resolve: None,
            load: load_op(d.color_load),
            store: StoreOp::store,
            clear: d.color_clear,
        });
        let targets = match (d.color, d.second) {
            (0, _) => 0,
            (_, 0) => 1,
            _ => 2,
        };
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
                colors: &colors[..targets],
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
        self.label(pass, "render", core::panic::Location::caller());
        pass
    }

    /// A compute pass, timed in `stamp`'s span where the frame is timed.
    #[track_caller]
    pub fn compute_pass(&self, encoder: Id, stamp: Option<encode::TimestampWrites>) -> Id {
        let s = self.s();
        let end = stamp.as_ref().and_then(|t| Some((t.query_set, t.end?)));
        let pass = handle_of(
            s.encoder
                .compute_pass(&mut s.stream, s.splits.at(encoder), &stamp),
        );
        s.splits.opened(pass, encoder, true, end);
        self.label(pass, "compute", core::panic::Location::caller());
        pass
    }

    pub fn dispatch(&self, pass: Id, x: u32, y: u32, z: u32) {
        let s = self.s();
        call::dispatch(&mut s.stream, s.splits.pass_at(pass), [x, y, z]);
        s.splits.ran(pass);
    }

    /// Dispatches the workgroups that `buf` holds at `off`, as counts across, up and deep.
    /// Only the GPU knows the counts, and they are written in a command buffer before the one
    /// this call is recorded in (gpu/split.rs).
    #[track_caller]
    pub fn dispatch_indirect(&self, pass: Id, buf: Id, off: u32) {
        let at = self.indirect(pass, buf);
        let s = self.s();
        call::dispatch_indirect(&mut s.stream, at, buf, off);
        s.splits.ran(pass);
    }

    #[track_caller]
    pub fn set_pipeline(&self, pass: Id, pipeline: Id) {
        let s = self.s();
        // A render pass that declares what it draws draws nothing else (frame/shader/sequence.rs
        // `Step::in_pass`). Any other pipeline is refused at the call's line, and the bug names
        // the pipeline's record and the pass.
        if let Some(record) = s.splits.undeclared(pass, pipeline) {
            xpute_kit::bug!(ENOTRECOVERABLE, u64::from(record), u64::from(pass));
        }
        call::set_pipeline(&mut s.stream, s.splits.pass_at(pass), pipeline);
        s.splits.piped(pass, pipeline);
    }

    /// Holds render pass `pass` to drawing only `pipelines`, given by their records' numbers as
    /// the frame's declarations name them (frame/shader/sequence.rs `Step::in_pass`).
    pub fn declare_pass(&self, pass: Id, pipelines: &'static [u32]) {
        self.s().splits.declared(pass, pipelines);
    }

    /// Notes that render pipeline `pipeline` was made from record `record`, so that a pass
    /// that declares what it draws can check the pipeline.
    pub fn recorded(&self, pipeline: Id, record: u32) {
        self.s().splits.recorded(pipeline, record);
    }

    /// Limits what `pass` draws to `rect`, `[x, y, width, height]` in its targets' pixels.
    pub fn set_scissor(&self, pass: Id, rect: [u32; 4]) {
        call::set_scissor(&mut self.s().stream, pass, rect);
    }

    #[track_caller]
    pub fn set_bind_group(&self, pass: Id, idx: u32, group: Id, offsets: &[u32]) {
        let s = self.s();
        // A whole render pass is one usage scope. A group that binds writable a buffer from
        // which one of the pass's indirect draws read its count is refused, as the draw is
        // when the group comes first (`indirect`), and the bug names them the same way.
        if let Some((buf, binding)) = s.splits.binds_read(pass, group) {
            xpute_kit::bug!(
                ENOTRECOVERABLE,
                u64::from(group) << 32 | u64::from(binding),
                u64::from(s.splits.made_at(buf)) << 32 | u64::from(buf)
            );
        }
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

    /// Submits the frame's encoder as its command buffers, each alone and in order. Where the
    /// frame is timed, wrote any span, and a read is free, the last command buffer also
    /// resolves the spans and copies them into that read's buffer, which is then mapped back.
    /// Before that, the earliest-asked read that is out is taken in if it landed, and it may
    /// then be asked again. A read's ticket is the order it was asked in, and its fence holds
    /// the ticket once it lands. Returns the read this frame's spans come back in.
    pub fn submit_frame(&self, encoder: Id, written: u64) -> Option<usize> {
        let s = self.s();
        s.stamps_complete = take_stamps(s);
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
        // the runtime's to write.
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

    /// Configures `surface` in the format the device prefers, opaque.
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

    /// Makes the spans' query set, two queries a span, with the buffer they resolve into and
    /// one mapped buffer per read. Returns the query set.
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

    /// How many objects are made and not yet let go, counted by the op that made each.
    pub fn objects(&self) -> &'static [u32; OBJECT_OPS] {
        self.s().encoder.objects()
    }

    /// Span `span`'s time in ms from the last read taken in.
    pub fn gpu_ms(&self, span: u32) -> f64 {
        self.s().gpu_ms[span as usize]
    }

    /// The time the last read's spans ran, in ms, with overlapping spans counted once.
    pub fn gpu_busy_ms(&self) -> f64 {
        self.s().gpu_busy_ms
    }

    /// The read that the frame just submitted took in, if it took one. The span times were
    /// measured from that read.
    pub fn stamps_complete(&self) -> Option<usize> {
        self.s().stamps_complete
    }

    /// Zeroes `size` bytes of buffer `id` from `at`, in order among `encoder`'s commands.
    pub fn clear_buf(&self, encoder: Id, id: Id, at: u32, size: u32) {
        let s = self.s();
        call::clear_buffer(&mut s.stream, s.splits.at(encoder), id, at, Some(size));
        s.splits.copied(encoder, id);
    }

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

    /// Maps `length` bytes of buffer `id` from `off` back into the core's memory at `addr`.
    /// `id` must be made `MAP_READ | COPY_DST` and copied into on an encoder submitted before.
    /// `fence` is the mapping's own fence and result. It holds `ticket` once the bytes are in
    /// or the mapping was refused, a turn or more later, and only then may `id` be mapped
    /// again (xpute MAP_ASYNC).
    pub fn map_read(
        &self,
        id: Id,
        off: u32,
        length: u32,
        addr: u32,
        fence: &[u32; 2],
        ticket: u32,
    ) {
        // SAFETY: the core's own range, which it asked to be written and
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

/// A label's text. Text past its room is cut off, not refused.
struct Label {
    bytes: [u8; 64],
    len: usize,
}

impl Default for Label {
    fn default() -> Self {
        Label {
            bytes: [0; 64],
            len: 0,
        }
    }
}

impl Label {
    fn text(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.len]).unwrap_or("")
    }
}

impl core::fmt::Write for Label {
    fn write_str(&mut self, text: &str) -> core::fmt::Result {
        let mut take = text.len().min(self.bytes.len() - self.len);
        while !text.is_char_boundary(take) {
            take -= 1;
        }
        self.bytes[self.len..self.len + take].copy_from_slice(&text.as_bytes()[..take]);
        self.len += take;
        Ok(())
    }
}
