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
    QueryType, SamplerBindingType, StorageTextureAccess, StoreOp, TextureAspect, TextureDimension,
    TextureFormat, TextureSampleType, TextureViewDimension, VertexFormat, VertexStepMode,
    FEATURE_NAME, LIMIT_NAME,
};
use xpute_gpu::spec::{BUFFER_USAGE, COLOR_WRITE, STATUS, TEXTURE_USAGE};
use xpute_guest::abi::handle::handle_slot;
use xpute_guest::global::Global;
use xpute_guest::ipc::stream::{Stream, WORD};
use xpute_guest::mem;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

pub use xpute_gpu::enums::TextureFormat as Format;
/// Set in a completion word once what it waits for is done.
pub use xpute_gpu::spec::DONE;

use crate::api_spec::{record, stream};
use crate::frame::shader::record::VERTEX_ENTRY;
use crate::gpu::span_spec::GPU_SPAN_NAMES;

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
/// each, the timestamp period, the preferred surface format, the status.
const FEAT_WORDS: usize = FEATURE_NAME.len().div_ceil(32);
const PERIOD_AT: usize = LIMIT_NAME.len() + FEAT_WORDS;
const DEVICE_WORDS: usize = PERIOD_AT + 3;

/// The head's words past the stream's own: the device's block, the spans'
/// read's word, then their begin and end ticks, two words a tick.
const DEVICE_AT: usize = 1;
const STAMPS_WORD_AT: usize = DEVICE_AT + DEVICE_WORDS;
const STAMPS_AT: usize = STAMPS_WORD_AT + 1;
const HEAD: usize = STAMPS_AT + 2 * 2 * SPANS;
const _: () = assert!(
    HEAD == stream::HEAD as usize,
    "spec/client/render/gpu/api.json's stream HEAD is the head laid out here"
);

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
pub struct Stamp {
    set: Id,
    begin: Option<u32>,
    end: u32,
}

impl Stamp {
    pub const NONE: Stamp = Stamp {
        set: 0,
        begin: None,
        end: 0,
    };

    pub fn of(set: Id, span: usize) -> Stamp {
        if set == 0 {
            return Stamp::NONE;
        }
        let begin = 2 * span as u32;
        Stamp {
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

/// How an attachment starts: cleared, kept, or (depth) read and not written.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Load {
    Clear,
    Load,
    ReadOnly,
}

/// A pass's targets: color cleared to transparent or loaded, depth cleared
/// to `depth_clear`, loaded or read only; 0 for one absent.
pub struct PassDesc {
    color: Id,
    color_load: Load,
    depth: Id,
    depth_load: Load,
    depth_clear: f32,
    stamp: Option<encode::TimestampWrites>,
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
        depth,
        depth_load,
        depth_clear,
        stamp,
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
pub const fn buf_range_entry(binding: u32, buf: Id, off: u32, sz: u32) -> Entry {
    Entry {
        binding,
        resource: encode::Resource::Buffer {
            buffer: buf,
            offset: off,
            size: Some(sz),
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
    unsafe { core::slice::from_raw_parts(lane.as_ptr().cast::<u8>(), core::mem::size_of_val(lane)) }
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
    asked: bool,
    device: Option<Device>,
    /// The spans' query set, its resolve buffer and the buffer it is copied
    /// into and mapped back through, which spans the read in flight timed,
    /// and their times from the last read in, in ms; past the last span, the
    /// whole frame's.
    stamps: Option<(Id, Id, Id)>,
    stamps_written: u32,
    /// Whether this frame asked the read: its mapping's ticket is written
    /// as it is encoded, so a frame dropped whole takes it back
    /// (`frame_commit`), or no read would be asked again.
    stamps_asked: bool,
    gpu_ms: [f64; SPANS],
    /// Whether the last frame submitted took a read in: its figures are then
    /// that read's, and else the read before it, not measured again.
    gpu_landed: bool,
    /// The images on their way into textures (`Gpu::image`).
    images: Vec<Arriving>,
    /// The images taken since `take_landed` last ran, on this device.
    landed: Vec<u32>,
    /// The devices gone so far: what was handed to the GPU under another
    /// count is on none there is.
    generation: u32,
}

/// An image file copied into its texture: the file's bytes kept until the
/// texture's word says it is there.
struct Arriving {
    image: u32,
    texture: Id,
    word: Box<u32>,
}

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

/// The encoder's bytes at the range's end, a word-multiple of 8.
const ENCODER_BYTES: usize = core::mem::size_of::<Encoder<OBJECTS>>().next_multiple_of(8);
const _: () = assert!(
    core::mem::align_of::<Encoder<OBJECTS>>() <= 8 && ENCODER_BYTES < stream::RESERVE as usize
);

/// # Safety
/// `at` must address `stream::BYTES`, zeroed, that nothing else writes,
/// 8-aligned, for as long as the module runs.
pub unsafe fn install(at: u32) -> u32 {
    let words = (stream::BYTES as usize - ENCODER_BYTES) / WORD;
    // SAFETY: the caller's range: the stream, then the encoder, which zeroed
    // bytes are an empty one of, aligned as the range is.
    let (stream, encoder) = unsafe {
        (
            Stream::new(mem::core(), at as usize, words, HEAD),
            &mut *mem::at::<Encoder<OBJECTS>>(at as usize + stream::BYTES as usize - ENCODER_BYTES),
        )
    };
    STATE.set(Some(State {
        stream,
        encoder,
        at,
        in_frame: false,
        asked: false,
        device: None,
        stamps: None,
        stamps_written: 0,
        stamps_asked: false,
        gpu_ms: [0.0; SPANS],
        gpu_landed: false,
        images: Vec::new(),
        landed: Vec::new(),
        generation: 0,
    }));
    at
}

/// Where the stream lies in the kernel's memory, once installed: what a host
/// runs after each turn.
pub fn stream_at() -> Option<u32> {
    STATE.get(|| None).as_ref().map(|s| s.at)
}

/// The renderer's calls, once the device has told what it is.
pub fn gpu() -> Option<&'static Gpu> {
    device().map(|_| &GPU)
}

/// The device as its block says, once the host has written it: asked for on
/// the first call after a device answers (`on_device_answered`).
pub fn device() -> Option<Device> {
    let s = STATE.get(|| None).as_mut()?;
    if s.device.is_none() && s.asked && *head(DEVICE_AT + DEVICE_WORDS - 1) == STATUS::READY {
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

/// A device answered: its block is asked for, and read once the host has
/// written it.
pub fn on_device_answered() {
    let s = state();
    *head(DEVICE_AT + DEVICE_WORDS - 1) = 0;
    let at = s.at as usize + DEVICE_AT * WORD;
    // SAFETY: the head's block, the host's to write.
    let block = unsafe { core::slice::from_raw_parts_mut(mem::at::<u32>(at), DEVICE_WORDS) };
    call::device(&mut s.stream, block);
    s.asked = true;
}

/// Whether the device went since this was last asked, as its host says in
/// the device's block (xpute STATUS.LOST); taken once.
pub fn take_device_lost() -> bool {
    if STATE.get(|| None).is_none() || *head(DEVICE_AT + DEVICE_WORDS - 1) != STATUS::LOST {
        return false;
    }
    *head(DEVICE_AT + DEVICE_WORDS - 1) = 0;
    true
}

/// The devices gone so far (`on_device_lost`): whoever hands the GPU anything
/// says under which, and what went under another is not there.
pub fn device_generation() -> u32 {
    STATE.get(|| None).as_ref().map_or(0, |s| s.generation)
}

/// The last read's ticks, where its word says they landed: each span's ms, and
/// the word let go for the next read; whether one landed.
fn take_stamps(s: &mut State) -> bool {
    let word = head(STAMPS_WORD_AT);
    if *word & DONE == 0 {
        return false;
    }
    *word = 0;
    let period_ms = f64::from(s.device.map_or(1.0, |d| d.period_ns)) / 1e6;
    let tick = |k: usize| {
        u64::from(*head(STAMPS_AT + 2 * k)) | u64::from(*head(STAMPS_AT + 2 * k + 1)) << 32
    };
    for k in 0..SPANS {
        let (b, e) = (tick(2 * k), tick(2 * k + 1));
        let ran = s.stamps_written & 1 << k != 0 && e >= b;
        s.gpu_ms[k] = if ran { (e - b) as f64 * period_ms } else { 0.0 };
    }
    true
}

/// The device went, and every object on it: the handles start over.
pub fn on_device_lost() {
    if let Some(s) = STATE.get(|| None).as_mut() {
        // SAFETY: the encoder's own bytes; zeroed, it has made nothing.
        unsafe { core::ptr::write_bytes(core::ptr::from_mut(s.encoder), 0, 1) };
        (s.asked, s.device, s.stamps) = (false, None, None);
        s.images.clear();
        s.landed.clear();
        s.generation = s.generation.wrapping_add(1);
        *head(STAMPS_WORD_AT) = 0;
    }
}

pub fn gpu_publish() {
    if let Some(s) = STATE.get(|| None).as_mut() {
        s.stream.publish();
    }
}

/// Each image whose texture was taken since this last ran, on the device
/// there is: its asker may let its file go, and hands it again only once
/// the device is lost.
pub fn take_landed(mut landed: impl FnMut(u32)) {
    if let Some(s) = STATE.get(|| None).as_mut() {
        s.landed.drain(..).for_each(&mut landed);
    }
}

/// Whether an image handed to the device has yet to land (`Gpu::image`):
/// another turn is wanted to take it.
pub fn images_landing() -> bool {
    STATE
        .get(|| None)
        .as_ref()
        .is_some_and(|s| s.images.iter().any(|a| *a.word & DONE == 0))
}

/// A frame's records are one: past the range they are dropped whole, with
/// the objects they made.
pub fn frame_begin() {
    let s = state();
    s.encoder.begin(&mut s.stream);
    s.in_frame = true;
    s.stamps_asked = false;
}

/// False where the frame did not fit and nothing of it was recorded: the
/// read it asked, never mapped, is freed for the next frame to ask.
pub fn frame_commit() -> bool {
    let s = state();
    s.in_frame = false;
    let committed = s.encoder.commit(&mut s.stream);
    if !committed && core::mem::take(&mut s.stamps_asked) {
        *head(STAMPS_WORD_AT) = 0;
    }
    committed
}

/// The bytes a write of `bytes` takes of the stream, as its writer measures
/// it: what a caller sums over the writes it makes together and asks `room`.
pub use xpute_gpu::encode::call::{write_buffer_bytes, write_texture_bytes};

/// Whether records of `bytes` (`write_buffer_bytes`, `write_texture_bytes`)
/// fit the stream, leaving `stream::RESERVE` for what cannot wait.
pub fn room(bytes: usize) -> bool {
    STATE
        .get(|| None)
        .as_ref()
        .is_some_and(|s| s.stream.fits(bytes, stream::RESERVE as usize))
}

/// An object's handle, or 0 where the frame it was made in no longer fits
/// (and is dropped with it); a full table is a bug.
fn made(id: Option<Id>) -> Id {
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
            s.encoder.destroy(&mut s.stream, id);
        }
    }
}

fn vertex_format(code: u32) -> VertexFormat {
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

impl Gpu {
    fn s(&self) -> &'static mut State {
        state()
    }

    /// A buffer of `size` bytes, one every device makes and, as storage,
    /// binds (frame/shader/floor.rs `fits`): a size past that is a bug the
    /// structure let through, not a device to fall short on.
    pub fn buf(&self, sz: u32, usage: u32) -> Id {
        xpute_kit::ensure!(
            crate::frame::shader::floor::fits(u64::from(sz), usage & buffer_usage::STORAGE != 0),
            ENOSPC,
            sz
        );
        let s = self.s();
        made(s.encoder.buffer(
            &mut s.stream,
            &encode::BufferDescriptor {
                size: sz,
                usage,
                mapped_at_creation: false,
            },
        ))
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
        made(s.encoder.texture(
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
    pub fn volume(&self, sz: [u32; 3], format: Format, usage: u32) -> Id {
        let s = self.s();
        made(s.encoder.texture(
            &mut s.stream,
            &encode::TextureDescriptor {
                size: sz,
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
        made(s.encoder.view(
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
        made(s.encoder.view(
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
        made(s.encoder.sampler(
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
        made(s.encoder.shader_module(&mut s.stream, code))
    }

    /// A group's (binding, stages, kind) words, as a pipeline record holds them.
    pub fn layout(&self, words: &[u32]) -> Id {
        let words = words.as_chunks::<3>().0;
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
        made(
            s.encoder
                .bind_group_layout(&mut s.stream, &entries[..words.len()]),
        )
    }

    fn pipeline_layout(&self, layouts: &[Id]) -> Id {
        let s = self.s();
        made(s.encoder.pipeline_layout(&mut s.stream, layouts))
    }

    /// A render pipeline from its record (frame/shader/record.rs) over the
    /// group layouts, drawing into `format`, depth32 its depth: into the
    /// depth alone where `format` is depth32's, its fragment writing no
    /// color (the shadow map, gpu/shadow.rs).
    pub fn pipeline(&self, words: &[u32], layouts: &[Id], module: Id, format: Format) -> Id {
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
        let bufs: Vec<encode::VertexBufferLayout> = lanes
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
        let p = made(s.encoder.render_pipeline(
            &mut s.stream,
            &encode::RenderPipelineDescriptor {
                layout: Some(layout),
                vertex: encode::VertexState {
                    module,
                    entry: VERTEX_ENTRY[words[record::HEAD as usize - 1] as usize],
                    buffers: &bufs,
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
                        format: TextureFormat::depth32float,
                        depth_write: words[4] != 0,
                        depth_compare: compare,
                        stencil_front: encode::STENCIL_UNUSED,
                        stencil_back: encode::STENCIL_UNUSED,
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
                    entry: "fs",
                    targets: if format == Format::depth32float {
                        &[]
                    } else {
                        &targets
                    },
                }),
            },
        ));
        s.encoder.destroy(&mut s.stream, layout);
        p
    }

    /// The module's compute entry `entry`, over `layouts`.
    pub fn compute_pipeline(&self, module: Id, entry: &str, layouts: &[Id]) -> Id {
        let layout = self.pipeline_layout(layouts);
        let s = self.s();
        let p = made(s.encoder.compute_pipeline(
            &mut s.stream,
            &encode::ComputePipelineDescriptor {
                layout: Some(layout),
                module,
                entry,
            },
        ));
        s.encoder.destroy(&mut s.stream, layout);
        p
    }

    pub fn bind_group(&self, layout: Id, entries: &[Entry]) -> Id {
        xpute_kit::ensure!(entries.len() <= GROUP_ENTRIES, ENOSPC, entries.len());
        let s = self.s();
        made(s.encoder.bind_group(&mut s.stream, layout, entries))
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

    /// `bytes` read where they are when the host runs the turn: their owner
    /// keeps them unwritten until then.
    pub fn upload_buf(&self, id: Id, off: u32, bytes: &[u8]) {
        call::write_buffer_at(&mut self.s().stream, id, off, bytes);
    }

    /// One layer's first mip, whole, four bytes a texel.
    pub fn write_texture(&self, id: Id, layer: u32, width: u32, height: u32, bytes: &[u8]) {
        let place = encode::TexelCopyTexture {
            texture: id,
            mip: 0,
            origin: [0, 0, layer],
            aspect: TextureAspect::all,
        };
        call::write_texture(
            &mut self.s().stream,
            &place,
            [width * 4, height],
            [width, height, 1],
            bytes,
        );
    }

    /// The image file at `file` (PNG, JPEG, WebP) decoded into `texture`, its
    /// `word` set to `ticket | DONE` once it is there.
    pub fn copy_image(&self, texture: Id, file: &[u8], word: &mut u32, ticket: u32) {
        let to = encode::TexelCopyTexture {
            texture,
            mip: 0,
            origin: [0, 0, 0],
            aspect: TextureAspect::all,
        };
        call::copy_external_image(&mut self.s().stream, &to, false, false, file, word, ticket);
    }

    /// The texture the image file `file`, named `image` by whoever hands it,
    /// is copied into, taken once it has landed: each image is copied in
    /// once, on this device, for the one that takes it. None while it
    /// lands, the first ask handing it to the device; none for good for a
    /// file whose size no header gives (pxl-spec `asset::image_sz`),
    /// which the kernel does not hand, and for the empty file of one
    /// already taken on this device.
    /// The file's bytes are its asker's until it is taken (`take_landed`);
    /// the host copies them out as it runs the turn.
    pub fn image(&self, image: u32, file: &'static [u8]) -> Option<Id> {
        let s = self.s();
        if let Some(k) = s.images.iter().position(|a| a.image == image) {
            if *s.images[k].word & DONE == 0 {
                return None;
            }
            s.landed.push(image);
            return Some(s.images.swap_remove(k).texture);
        }
        let (width, height) = pxl_spec::asset::image_sz(file)?;
        let usage = TEXTURE_USAGE::TEXTURE_BINDING
            | TEXTURE_USAGE::COPY_SRC
            | TEXTURE_USAGE::COPY_DST
            | TEXTURE_USAGE::RENDER_ATTACHMENT;
        let texture = self.texture(width, height, 1, Format::rgba8unorm_srgb, 1, usage);
        let mut word = Box::new(0);
        self.copy_image(texture, file, &mut word, image & !DONE | 1);
        self.s().images.push(Arriving {
            image,
            texture,
            word,
        });
        None
    }

    pub fn encoder(&self) -> Id {
        let s = self.s();
        made(s.encoder.encoder(&mut s.stream))
    }

    /// A region of `source`'s first mip into `target`'s `layer`.
    #[allow(clippy::too_many_arguments)]
    pub fn copy_texture(
        &self,
        encoder: Id,
        src: Id,
        x: u32,
        y: u32,
        target: Id,
        tx: u32,
        ty: u32,
        layer: u32,
        width: u32,
        height: u32,
    ) {
        let place = |texture, origin| encode::TexelCopyTexture {
            texture,
            mip: 0,
            origin,
            aspect: TextureAspect::all,
        };
        call::copy_texture_to_texture(
            &mut self.s().stream,
            encoder,
            &place(src, [x, y, 0]),
            &place(target, [tx, ty, layer]),
            [width, height, 1],
        );
    }

    pub fn pass(&self, encoder: Id, d: &PassDesc) -> Id {
        let colors = [encode::ColorAttachment {
            view: d.color,
            resolve: None,
            load: load_op(d.color_load),
            store: StoreOp::store,
            clear: [0.0; 4],
        }];
        let depth_stencil = (d.depth != 0).then(|| encode::DepthStencilAttachment {
            view: d.depth,
            depth_load: load_op(d.depth_load),
            depth_store: StoreOp::store,
            depth_clear: d.depth_clear,
            depth_read_only: d.depth_load == Load::ReadOnly,
            stencil_load: LoadOp::load,
            stencil_store: StoreOp::store,
            stencil_clear: 0,
            stencil_read_only: false,
        });
        let s = self.s();
        made(s.encoder.render_pass(
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
        ))
    }

    /// Timed in `stamp`'s span where the frame is timed.
    pub fn compute_pass(&self, encoder: Id, stamp: Option<encode::TimestampWrites>) -> Id {
        let s = self.s();
        made(s.encoder.compute_pass(&mut s.stream, encoder, &stamp))
    }

    pub fn dispatch(&self, pass: Id, x: u32, y: u32, z: u32) {
        call::dispatch(&mut self.s().stream, pass, [x, y, z]);
    }

    /// Workgroups as `buffer` holds them from `offset`, across, up and deep:
    /// a count only the GPU knows.
    pub fn dispatch_indirect(&self, pass: Id, buf: Id, off: u32) {
        call::dispatch_indirect(&mut self.s().stream, pass, buf, off);
    }

    pub fn set_pipeline(&self, pass: Id, pipeline: Id) {
        call::set_pipeline(&mut self.s().stream, pass, pipeline);
    }

    /// What the pass draws held to `[x, y, width, height]` of its targets.
    pub fn set_scissor(&self, pass: Id, rect: [u32; 4]) {
        call::set_scissor(&mut self.s().stream, pass, rect);
    }

    pub fn set_bind_group(&self, pass: Id, idx: u32, group: Id, offsets: &[u32]) {
        call::set_bind_group(&mut self.s().stream, pass, idx, group, offsets);
    }

    /// A size of 0 is the rest of the buffer.
    pub fn set_vertex_buf(&self, pass: Id, slot: u32, buf: Id, off: u32, sz: u32) {
        call::set_vertex_buffer(
            &mut self.s().stream,
            pass,
            slot,
            buf,
            off,
            (sz != 0).then_some(sz),
        );
    }

    /// Indices are u32.
    pub fn set_idx_buf(&self, pass: Id, buf: Id, off: u32, sz: u32) {
        call::set_index_buffer(
            &mut self.s().stream,
            pass,
            buf,
            IndexFormat::uint32,
            off,
            (sz != 0).then_some(sz),
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
        call::draw(
            &mut self.s().stream,
            pass,
            vertices,
            instances,
            first_vertex,
            first_instance,
        );
    }

    pub fn draw_indexed(
        &self,
        pass: Id,
        idxs: u32,
        instances: u32,
        first_idx: u32,
        first_instance: u32,
    ) {
        call::draw_indexed(
            &mut self.s().stream,
            pass,
            idxs,
            instances,
            first_idx,
            0,
            first_instance,
        );
    }

    pub fn draw_indirect(&self, pass: Id, buf: Id, off: u32) {
        call::draw_indirect(&mut self.s().stream, pass, buf, off);
    }

    pub fn draw_indexed_indirect(&self, pass: Id, buf: Id, off: u32) {
        call::draw_indexed_indirect(&mut self.s().stream, pass, buf, off);
    }

    /// Ends the pass and lets its handle go.
    pub fn end(&self, pass: Id) {
        let s = self.s();
        s.encoder.end(&mut s.stream, pass);
    }

    /// Submits the encoder and lets its handle go.
    pub fn submit(&self, encoder: Id) {
        let s = self.s();
        s.encoder.submit(&mut s.stream, &[encoder]);
    }

    /// The frame's encoder, its spans resolved, copied out and mapped back
    /// where it wrote any and no mapping is still on its way, submitted. One
    /// mapping at most is on its way, so its ticket need not tell them apart.
    /// A mapping that landed is taken in first: asking for the next writes its
    /// ticket over the word that says so. Whether this frame's spans are on
    /// their way back.
    pub fn submit_frame(&self, encoder: Id, written: u32) -> bool {
        let s = self.s();
        s.gpu_landed = take_stamps(s);
        let stamps_free = *head(STAMPS_WORD_AT) == 0;
        let asked = stamps_free && written != 0;
        if let (Some((set, resolve, mapped)), true) = (s.stamps, asked) {
            let bytes = 2 * SPANS as u32 * 8;
            call::resolve_query_set(&mut s.stream, encoder, set, 0, 2 * SPANS as u32, resolve, 0);
            call::copy_buffer_to_buffer(&mut s.stream, encoder, (resolve, 0), (mapped, 0), bytes);
            s.encoder.submit(&mut s.stream, &[encoder]);
            s.stamps_written = written;
            let at = s.at as usize + STAMPS_AT * WORD;
            // SAFETY: the head's ticks and their word, the host's to write.
            let (ticks, word) = unsafe {
                (
                    core::slice::from_raw_parts_mut(mem::at::<u8>(at), 2 * SPANS * 8),
                    &mut *mem::at::<u32>(s.at as usize + STAMPS_WORD_AT * WORD),
                )
            };
            call::map_async(&mut s.stream, mapped, 0, ticks, word, 1);
            s.stamps_asked = true;
            true
        } else {
            s.encoder.submit(&mut s.stream, &[encoder]);
            false
        }
    }

    /// Configured in the format the device prefers, its alpha premultiplied.
    pub fn configure(&self, surface: u32) {
        call::configure(
            &mut self.s().stream,
            handle_slot(surface),
            None,
            TEXTURE_USAGE::RENDER_ATTACHMENT,
            CanvasAlphaMode::premultiplied,
        );
    }

    pub fn resize(&self, surface: u32, width: u32, height: u32) {
        call::resize(&mut self.s().stream, handle_slot(surface), [width, height]);
    }

    /// A view of the surface's texture for this frame.
    pub fn cur(&self, surface: u32) -> Id {
        let s = self.s();
        made(s.encoder.current(&mut s.stream, handle_slot(surface)))
    }

    /// The spans' query set, two queries a span, the buffer they resolve into
    /// and the one that is mapped back.
    pub fn spans(&self) -> Id {
        let s = self.s();
        let set = made(
            s.encoder
                .query_set(&mut s.stream, QueryType::timestamp, 2 * SPANS as u32),
        );
        let resolve = made(s.encoder.buffer(
            &mut s.stream,
            &encode::BufferDescriptor {
                size: (2 * SPANS * 8) as u32,
                usage: BUFFER_USAGE::QUERY_RESOLVE | BUFFER_USAGE::COPY_SRC,
                mapped_at_creation: false,
            },
        ));
        let mapped = made(s.encoder.buffer(
            &mut s.stream,
            &encode::BufferDescriptor {
                size: (2 * SPANS * 8) as u32,
                usage: BUFFER_USAGE::MAP_READ | BUFFER_USAGE::COPY_DST,
                mapped_at_creation: false,
            },
        ));
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

    /// Whether the frame just submitted took a read in, so its figures were
    /// measured with it.
    pub fn has_gpu_landed(&self) -> bool {
        self.s().gpu_landed
    }

    /// `size` bytes of `from` from `at` into `to` from `to_at`, on `encoder`.
    pub fn copy_buf(&self, encoder: Id, (from, at): (Id, u32), (to, to_at): (Id, u32), sz: u32) {
        call::copy_buffer_to_buffer(&mut self.s().stream, encoder, (from, at), (to, to_at), sz);
    }

    /// `length` bytes of `id`, a buffer made `MAP_READ | COPY_DST` and copied
    /// into on an encoder submitted before, mapped back from `offset` into
    /// the kernel's memory at `address`; `word`, the mapping's own, holds
    /// `ticket | DONE` once they are in, a turn or more later, and only then
    /// may `id` be mapped again (xpute MAP_ASYNC).
    pub fn map_read(&self, id: Id, off: u32, length: u32, addr: u32, word: &mut u32, ticket: u32) {
        // SAFETY: the kernel's own range, which it asked to be written and
        // does not read while the mapping is out.
        let into = unsafe {
            core::slice::from_raw_parts_mut(mem::at::<u8>(addr as usize), length as usize)
        };
        call::map_async(&mut self.s().stream, id, off, into, word, ticket);
    }
}
