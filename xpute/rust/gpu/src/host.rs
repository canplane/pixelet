// xpute-gpu/host.rs

//! The host's end of a guest's GPU calls on a native device: runs a
//! stream's records (spec/gpu/api.json) on a wgpu device, each object kept at
//! its handle's slot, each enumeration's number turned into wgpu's value by
//! its WebGPU name, the counterpart of ts/gpu/src/host.ts. It is told the
//! guest's memory, the device and the surfaces the program binds, and
//! nothing of what the guest draws.
//!
//! What finishes after a turn, a buffer read back and the queue's work done,
//! lands when the device is polled at a later turn's edge (`poll`): the
//! guest finds its word at `ticket | DONE` a turn or more later, as on the
//! web, and nothing here waits.

use core::marker::PhantomData;
use core::ptr::NonNull;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};
use std::sync::Arc;

use serde::de::value::{Error as NameError, StrDeserializer};
use serde::de::{DeserializeOwned, IntoDeserializer};
use xpute_guest::abi::handle::handle_slot;

use crate::enums::{
    ADDRESS_MODE, BLEND_FACTOR, BLEND_OPERATION, BUFFER_BINDING_TYPE, CANVAS_ALPHA_MODE, COMPARE_FUNCTION, CULL_MODE, FEATURE_NAME, FILTER_MODE, FRONT_FACE, INDEX_FORMAT, LIMIT_NAME, LOAD_OP,
    MIPMAP_FILTER_MODE, PRIMITIVE_TOPOLOGY, QUERY_TYPE, SAMPLER_BINDING_TYPE, STENCIL_OPERATION, STORAGE_TEXTURE_ACCESS, STORE_OP, TEXTURE_ASPECT, TEXTURE_DIMENSION, TEXTURE_FORMAT,
    TEXTURE_SAMPLE_TYPE, TEXTURE_VIEW_DIMENSION, VERTEX_FORMAT, VERTEX_STEP_MODE,
};
use crate::spec::{op, BUFFER_USAGE, COLOR_WRITE, DONE, NONE, SHADER_STAGE, STATUS, SURFACE_PREFERRED, TEXTURE_USAGE};

enum Object {
    Buffer(wgpu::Buffer),
    Texture(wgpu::Texture),
    /// With its format, which a pass's depth attachment asks of it.
    View(wgpu::TextureView, wgpu::TextureFormat),
    Sampler(wgpu::Sampler),
    Shader(wgpu::ShaderModule),
    BindGroupLayout(wgpu::BindGroupLayout),
    PipelineLayout(wgpu::PipelineLayout),
    BindGroup(wgpu::BindGroup),
    RenderPipeline(wgpu::RenderPipeline),
    ComputePipeline(wgpu::ComputePipeline),
    QuerySet(wgpu::QuerySet),
    Encoder(wgpu::CommandEncoder),
    /// Boxed, so a slot is a handle's size and not a pass's.
    RenderPass(Box<wgpu::RenderPass<'static>>),
    ComputePass(Box<wgpu::ComputePass<'static>>),
}

/// Each enumeration's members as wgpu's values, by their place in the
/// spec's list: the WebGPU name read by wgpu's own names for it, made once.
/// None for a name wgpu does not give the value of.
struct Names {
    texture_format: Vec<Option<wgpu::TextureFormat>>,
    vertex_format: Vec<Option<wgpu::VertexFormat>>,
    topology: Vec<Option<wgpu::PrimitiveTopology>>,
    index_format: Vec<Option<wgpu::IndexFormat>>,
    front_face: Vec<Option<wgpu::FrontFace>>,
    compare: Vec<Option<wgpu::CompareFunction>>,
    stencil: Vec<Option<wgpu::StencilOperation>>,
    blend_operation: Vec<Option<wgpu::BlendOperation>>,
    blend_factor: Vec<Option<wgpu::BlendFactor>>,
    step: Vec<Option<wgpu::VertexStepMode>>,
    address: Vec<Option<wgpu::AddressMode>>,
    filter: Vec<Option<wgpu::FilterMode>>,
    mipmap: Vec<Option<wgpu::MipmapFilterMode>>,
    dimension: Vec<Option<wgpu::TextureDimension>>,
    view_dimension: Vec<Option<wgpu::TextureViewDimension>>,
    aspect: Vec<Option<wgpu::TextureAspect>>,
    sampler_binding: Vec<Option<wgpu::SamplerBindingType>>,
    storage_access: Vec<Option<wgpu::StorageTextureAccess>>,
}

fn named<T: DeserializeOwned>(names: &[&str]) -> Vec<Option<T>> {
    names
        .iter()
        .map(|n| {
            let d: StrDeserializer<NameError> = n.into_deserializer();
            T::deserialize(d).ok()
        })
        .collect()
}

impl Names {
    fn new() -> Names {
        Names {
            texture_format: named(TEXTURE_FORMAT),
            vertex_format: named(VERTEX_FORMAT),
            topology: named(PRIMITIVE_TOPOLOGY),
            index_format: named(INDEX_FORMAT),
            front_face: named(FRONT_FACE),
            compare: named(COMPARE_FUNCTION),
            stencil: named(STENCIL_OPERATION),
            blend_operation: named(BLEND_OPERATION),
            blend_factor: named(BLEND_FACTOR),
            step: named(VERTEX_STEP_MODE),
            address: named(ADDRESS_MODE),
            filter: named(FILTER_MODE),
            mipmap: named(MIPMAP_FILTER_MODE),
            dimension: named(TEXTURE_DIMENSION),
            view_dimension: named(TEXTURE_VIEW_DIMENSION),
            aspect: named(TEXTURE_ASPECT),
            sampler_binding: named(SAMPLER_BINDING_TYPE),
            storage_access: named(STORAGE_TEXTURE_ACCESS),
        }
    }
}

/// Each flag set's members as wgpu's flags of the same name: a guest's word
/// is the union of the flags of the members whose bits it sets, whatever bits
/// wgpu gives them (its trunk splits them over two words).
const BUFFER_USAGES: [(u32, wgpu::BufferUsages); 10] = [
    (BUFFER_USAGE::MAP_READ, wgpu::BufferUsages::MAP_READ),
    (BUFFER_USAGE::MAP_WRITE, wgpu::BufferUsages::MAP_WRITE),
    (BUFFER_USAGE::COPY_SRC, wgpu::BufferUsages::COPY_SRC),
    (BUFFER_USAGE::COPY_DST, wgpu::BufferUsages::COPY_DST),
    (BUFFER_USAGE::INDEX, wgpu::BufferUsages::INDEX),
    (BUFFER_USAGE::VERTEX, wgpu::BufferUsages::VERTEX),
    (BUFFER_USAGE::UNIFORM, wgpu::BufferUsages::UNIFORM),
    (BUFFER_USAGE::STORAGE, wgpu::BufferUsages::STORAGE),
    (BUFFER_USAGE::INDIRECT, wgpu::BufferUsages::INDIRECT),
    (BUFFER_USAGE::QUERY_RESOLVE, wgpu::BufferUsages::QUERY_RESOLVE),
];
const TEXTURE_USAGES: [(u32, wgpu::TextureUsages); 5] = [
    (TEXTURE_USAGE::COPY_SRC, wgpu::TextureUsages::COPY_SRC),
    (TEXTURE_USAGE::COPY_DST, wgpu::TextureUsages::COPY_DST),
    (TEXTURE_USAGE::TEXTURE_BINDING, wgpu::TextureUsages::TEXTURE_BINDING),
    (TEXTURE_USAGE::STORAGE_BINDING, wgpu::TextureUsages::STORAGE_BINDING),
    (TEXTURE_USAGE::RENDER_ATTACHMENT, wgpu::TextureUsages::RENDER_ATTACHMENT),
];
const SHADER_STAGES: [(u32, wgpu::ShaderStages); 3] = [
    (SHADER_STAGE::VERTEX, wgpu::ShaderStages::VERTEX),
    (SHADER_STAGE::FRAGMENT, wgpu::ShaderStages::FRAGMENT),
    (SHADER_STAGE::COMPUTE, wgpu::ShaderStages::COMPUTE),
];
const COLOR_WRITES: [(u32, wgpu::ColorWrites); 5] = [
    (COLOR_WRITE::RED, wgpu::ColorWrites::RED),
    (COLOR_WRITE::GREEN, wgpu::ColorWrites::GREEN),
    (COLOR_WRITE::BLUE, wgpu::ColorWrites::BLUE),
    (COLOR_WRITE::ALPHA, wgpu::ColorWrites::ALPHA),
    (COLOR_WRITE::ALL, wgpu::ColorWrites::ALL),
];
// A member the spec gains is one these name, or the build stops here.
const _: () = assert!(
    BUFFER_USAGES.len() == BUFFER_USAGE::KEYS.len()
        && TEXTURE_USAGES.len() == TEXTURE_USAGE::KEYS.len()
        && SHADER_STAGES.len() == SHADER_STAGE::KEYS.len()
        && COLOR_WRITES.len() == COLOR_WRITE::KEYS.len()
);

/// The flags of `of`'s members whose bits `word` sets.
fn flags<F: Copy + core::ops::BitOr<Output = F>>(of: &[(u32, F)], none: F, word: u32) -> F {
    of.iter().filter(|(bit, _)| word & bit == *bit).fold(none, |all, (_, f)| all | *f)
}

/// `n` of `of`, a member the guest named; a name wgpu gives no value of is
/// the guest asking for what this device cannot be, which is a bug of the
/// program, not of the host.
fn of<T: Copy>(of: &[Option<T>], n: u32, what: &str) -> T {
    of.get(n as usize).copied().flatten().unwrap_or_else(|| panic!("[gpu] {what} {n} has no wgpu value"))
}

/// A record's words read in order, as the guest wrote them.
struct Words<'w> {
    w: &'w [u32],
    at: usize,
}

impl<'w> Words<'w> {
    fn u(&mut self) -> u32 {
        let v = self.w[self.at];
        self.at += 1;
        v
    }
    fn i(&mut self) -> i32 {
        self.u() as i32
    }
    fn f32(&mut self) -> f32 {
        f32::from_bits(self.u())
    }
    fn flag(&mut self) -> bool {
        self.u() != 0
    }
    /// A count, then that many words, read where they lie.
    fn list(&mut self) -> &'w [u32] {
        let n = self.u() as usize;
        let words = &self.w[self.at..self.at + n];
        self.at += n;
        words
    }
    /// None where the guest left it unset.
    fn opt(&mut self) -> Option<u32> {
        Some(self.u()).filter(|&v| v != NONE)
    }
    /// A count, then the bytes padded to words.
    fn bytes(&mut self) -> &'w [u8] {
        let n = self.u() as usize;
        let words = &self.w[self.at..self.at + n.div_ceil(4)];
        self.at += n.div_ceil(4);
        // SAFETY: the words as their bytes, which are the guest's in order on
        // a little-endian host, as every host is; at least `n` of them.
        let all: &'w [u8] = unsafe { core::slice::from_raw_parts(words.as_ptr().cast::<u8>(), 4 * words.len()) };
        &all[..n]
    }
    fn text(&mut self) -> String {
        String::from_utf8_lossy(self.bytes()).into_owned()
    }
}

/// A read or a done the device answers at a later poll.
struct Pending {
    state: Arc<AtomicU8>,
    word: u32,
    ticket: u32,
    /// A mapping's buffer, its range and where its bytes go: its word is
    /// its own, written whatever it holds (MAP_ASYNC).
    read: Option<(wgpu::Buffer, u64, u64, u32)>,
}

const WAITING: u8 = 0;
const ANSWERED: u8 = 1;
const REFUSED: u8 = 2;

/// An image the decoder has (COPY_EXTERNAL_IMAGE), landed in its texture at
/// the poll after its decode comes back.
struct Decoding {
    /// Its place in what was sent the decoder, which answers in order.
    job: u64,
    to: OwnedTexture,
    word: u32,
    ticket: u32,
}

/// What the decoder is asked: a job's number, the bytes, flip y and premultiply.
type Job = (u64, Vec<u8>, bool, bool);

/// The one thread decoding images off the turn, made with the host, its
/// answers in the order it was asked.
struct Decoder {
    ask: std::sync::mpsc::Sender<Job>,
    answers: std::sync::mpsc::Receiver<(u64, Option<crate::image::Image>)>,
    next: u64,
}

impl Decoder {
    /// Each answer `wake`s the host's loop, from this thread: the image lands
    /// at the next turn's poll, which nothing else would ask for.
    fn new(wake: fn()) -> Decoder {
        let (ask, jobs) = std::sync::mpsc::channel::<Job>();
        let (answer, answers) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for (job, bytes, flip_y, premultiplied) in jobs {
                if answer.send((job, crate::image::decode(&bytes, flip_y, premultiplied))).is_err() {
                    return;
                }
                wake();
            }
        });
        Decoder { ask, answers, next: 0 }
    }
}

/// A surface the guest names by number: what the guest asked of it, which
/// holds before a window's surface is bound to it and after, and that surface
/// while one is.
struct Surface {
    /// CONFIGURE's: format, usage and whether the guest's alpha is
    /// premultiplied; none before it.
    asked: Option<(wgpu::TextureFormat, wgpu::TextureUsages, bool)>,
    /// RESIZE's, at least a pixel each way.
    size: [u32; 2],
    bound: Option<Bound>,
    /// This turn's texture, the same to every CURRENT of the turn.
    frame: Option<Frame>,
    /// The texture drawn where the window gives none, kept until what is
    /// asked of the surface changes, so a hidden window costs no texture a frame.
    unshown: Option<wgpu::Texture>,
}

struct Bound {
    surface: wgpu::Surface<'static>,
    /// What is asked differs from what the surface was last configured with,
    /// so no frame is taken from it until it is configured (`configure_surfaces`).
    stale: bool,
}

/// The window's texture, shown at the turn's end; or, where the window gives
/// none this turn, one of its format and size drawn into and never shown, as
/// WebGPU's getCurrentTexture always gives a texture.
enum Frame {
    Shown(wgpu::SurfaceTexture),
    Unshown(wgpu::Texture),
}

impl Default for Surface {
    fn default() -> Self {
        Surface {
            asked: None,
            size: [1, 1],
            bound: None,
            frame: None,
            unshown: None,
        }
    }
}

impl Frame {
    fn texture(&self) -> &wgpu::Texture {
        match self {
            Frame::Shown(f) => &f.texture,
            Frame::Unshown(t) => t,
        }
    }
}

/// The frames a surface may have drawn and not yet finished on the GPU: what
/// it is configured to queue (`desired_maximum_frame_latency`), and what
/// `GpuHost::admit` lets stand. A surface keeps one texture more than this,
/// so with this many unfinished one is still free, and taking it never
/// blocks the thread that turns the guest.
pub const FRAME_LATENCY: u32 = 2;

/// The frames admitted and not yet done on the GPU. The queue's callback
/// takes one off, run by the host's poll; it captures nothing, so handing it
/// over allocates nothing each frame. One count for the process: two hosts
/// that admit frames hold each other back, and neither miscounts.
static UNFINISHED: AtomicU32 = AtomicU32::new(0);

/// Where a stale surface is configured: configuring one sets its layer's
/// properties, which some platforms allow on one thread alone. `Elsewhere`
/// asks for it there, where the caller then runs `configure_surfaces`.
#[derive(Clone, Copy)]
pub enum ConfigureAt {
    Here,
    Elsewhere(fn()),
}

pub struct GpuHost {
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    names: Names,
    objects: Vec<Option<Object>>,
    /// At the number the program binds each by, as objects at their slots.
    surfaces: Vec<Surface>,
    /// The surface format the device prefers, which SURFACE_PREFERRED names
    /// and DEVICE tells the guest, as the web's getPreferredCanvasFormat.
    preferred: wgpu::TextureFormat,
    configure_at: ConfigureAt,
    /// Where the guest asked the device's block written, for its status.
    block: Option<(u32, u32)>,
    /// The device went, as wgpu said from its own thread; taken at the
    /// turn's edge (`poll`).
    gone: Arc<AtomicBool>,
    pending: Vec<Pending>,
    decoder: Decoder,
    decoding: std::collections::VecDeque<Decoding>,
    /// A record's words, copied out of the guest's memory so the record may
    /// write into it.
    scratch: Vec<u32>,
    /// A submit's command buffers, gathered in place each time.
    finished: Vec<wgpu::CommandBuffer>,
    /// This turn's frame was admitted, so it is counted once presented.
    admitted: bool,
    /// Every buffer write goes through the belt, whose chunks are mapped once
    /// and reused, into `uploads`, which goes ahead of whatever is submitted
    /// next: `Queue::write_buffer` makes a Metal buffer and mapping a write.
    belt: wgpu::util::StagingBelt,
    uploads: Option<wgpu::CommandEncoder>,
}

/// A mesh, the largest write a frame makes, is about a MiB; larger writes take
/// a chunk of their own.
const BELT_CHUNK_BYTES: u64 = 1 << 20;

/// The guest's core memory, as the host reads and writes it: by the offsets
/// the guest names, each read or write reaching only the range it names, and
/// never one reference over the whole. The host's own blocks may lie in the
/// same memory — a guest linked into the host's process allocates for both
/// from one heap — and a `&mut [u8]` over all of it would claim them too.
pub struct Memory<'a> {
    base: NonNull<u8>,
    bytes: usize,
    of: PhantomData<&'a mut [u8]>,
}

impl<'a> Memory<'a> {
    /// A memory the caller holds whole, as a test does.
    pub fn of(bytes: &'a mut [u8]) -> Memory<'a> {
        Memory {
            bytes: bytes.len(),
            base: NonNull::from(bytes).cast(),
            of: PhantomData,
        }
    }

    /// The memory at `base`, `bytes` long.
    ///
    /// # Safety
    /// The `bytes` bytes at `base` are readable and writable for as long as
    /// the value is used. While it is, the caller holds no reference into a
    /// range the guest names, and no other thread reads or writes one: the
    /// guest is between turns, and what else lies in the memory (the host's
    /// own blocks among it) is reached only through its owners.
    pub unsafe fn from_raw(base: NonNull<u8>, bytes: usize) -> Memory<'a> {
        Memory { base, bytes, of: PhantomData }
    }

    /// The `n` bytes at `at`; past the memory's end is the guest's bug.
    fn range(&self, at: usize, n: usize) -> *mut u8 {
        let end = at.checked_add(n).filter(|&end| end <= self.bytes);
        assert!(end.is_some(), "[gpu] {n} bytes at {at} run past the memory's {}", self.bytes);
        // SAFETY: in bounds, as just checked.
        unsafe { self.base.as_ptr().add(at) }
    }

    /// The bytes at `at`, borrowed while nothing is written.
    fn read(&self, at: usize, n: usize) -> &[u8] {
        // SAFETY: in the memory, which `from_raw`'s caller lets this read;
        // `&self` keeps every write out while the slice lives.
        unsafe { core::slice::from_raw_parts(self.range(at, n), n) }
    }

    fn write(&mut self, at: usize, from: &[u8]) {
        let to = self.range(at, from.len());
        // SAFETY: in the memory; `from` may lie in it too (the host's own
        // blocks can), so the copy allows an overlap.
        unsafe { core::ptr::copy(from.as_ptr(), to, from.len()) }
    }

    fn word(&self, at: u32) -> u32 {
        let w = self.read(at as usize, 4);
        u32::from_le_bytes([w[0], w[1], w[2], w[3]])
    }

    fn set_word(&mut self, at: u32, v: u32) {
        self.write(at as usize, &v.to_le_bytes());
    }
}

impl GpuHost {
    /// `preferred` the surface format the platform shows best, `configure_at`
    /// where its surfaces may be configured. `wake` asks the platform's loop
    /// for a turn, from any thread: a completion is written into the guest's
    /// memory only at a turn's poll, so whoever learns of one between turns
    /// asks for it, as an executor's pender asks for a poll
    /// (embassy-executor's `Pender`; its std one signals the sleeping loop).
    ///
    /// Its device's loss is its own to hear: wgpu calls back, from any thread,
    /// for a loss of the device's (`Unknown`), and for its destruction or drop
    /// by its owner (`Destroyed`), which is no loss; the host tells the guest
    /// in its block at the next turn's edge and asks for that turn.
    pub fn new(adapter: wgpu::Adapter, device: wgpu::Device, queue: wgpu::Queue, preferred: wgpu::TextureFormat, configure_at: ConfigureAt, wake: fn()) -> GpuHost {
        let gone = Arc::new(AtomicBool::new(false));
        let lost = gone.clone();
        device.set_device_lost_callback(move |reason, _| {
            if reason == wgpu::DeviceLostReason::Unknown {
                lost.store(true, Ordering::Release);
                wake();
            }
        });
        GpuHost {
            adapter,
            belt: wgpu::util::StagingBelt::new(device.clone(), BELT_CHUNK_BYTES),
            device,
            queue,
            names: Names::new(),
            objects: Vec::new(),
            surfaces: Vec::new(),
            preferred,
            configure_at,
            block: None,
            gone,
            pending: Vec::new(),
            decoder: Decoder::new(wake),
            decoding: std::collections::VecDeque::new(),
            scratch: Vec::new(),
            finished: Vec::new(),
            admitted: false,
            uploads: None,
        }
    }

    /// The window's surface for the guest's `surface`, configured as the
    /// guest asked once it is (`configure_at`), at the size it last gave.
    pub fn bind_surface(&mut self, surface: u32, s: wgpu::Surface<'static>) {
        self.surface(surface).bound = Some(Bound { surface: s, stale: true });
        self.want_configured(surface);
    }

    /// The guest's `surface` drawn, until a surface is bound again, into
    /// frames never shown.
    pub fn unbind_surface(&mut self, surface: u32) {
        let s = self.surface(surface);
        s.bound = None;
        s.frame = None;
    }

    /// Every bound surface whose configuration is not what is asked,
    /// configured: run where `configure_at` says, while nothing else uses the
    /// host.
    pub fn configure_surfaces(&mut self) {
        for (n, s) in self.surfaces.iter_mut().enumerate() {
            let (Some(b), Some((format, usage, premultiplied))) = (s.bound.as_mut().filter(|b| b.stale), s.asked) else {
                continue;
            };
            let caps = b.surface.get_capabilities(&self.adapter);
            // A format the surface does not offer is the adapter unable to show it there.
            assert!(caps.formats.contains(&format), "[gpu] surface {n} cannot show {format:?} on this adapter");
            // Shows through where clear, as a premultiplied canvas does; Metal
            // offers only opaque and post-multiplied.
            let alpha_mode = [wgpu::CompositeAlphaMode::PreMultiplied, wgpu::CompositeAlphaMode::PostMultiplied]
                .into_iter()
                .find(|m| premultiplied && caps.alpha_modes.contains(m))
                .unwrap_or(wgpu::CompositeAlphaMode::Opaque);
            let config = wgpu::SurfaceConfiguration {
                usage,
                format,
                color_space: wgpu::SurfaceColorSpace::Auto,
                width: s.size[0],
                height: s.size[1],
                present_mode: wgpu::PresentMode::Fifo,
                desired_maximum_frame_latency: FRAME_LATENCY,
                alpha_mode,
                view_formats: Vec::new(),
            };
            b.surface.configure(&self.device, &config);
            b.stale = false;
        }
    }

    /// `surface`'s texture for this turn: the window's where it gives one, else
    /// one never shown. A surface the window has outdated is configured and
    /// asked again, where it may be configured here; one it gives suboptimal
    /// is shown and configured before the next.
    fn current(&mut self, surface: u32) -> Frame {
        let mut asked = false;
        loop {
            if let ConfigureAt::Here = self.configure_at {
                self.configure_surfaces();
            }
            let Some(b) = self.surface(surface).bound.as_mut().filter(|b| !b.stale) else { break };
            match b.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(f) => return Frame::Shown(f),
                wgpu::CurrentSurfaceTexture::Suboptimal(f) => {
                    b.stale = true;
                    if let ConfigureAt::Elsewhere(ask) = self.configure_at {
                        ask();
                    }
                    return Frame::Shown(f);
                }
                wgpu::CurrentSurfaceTexture::Outdated if !asked => {
                    asked = true;
                    self.want_configured(surface);
                }
                _ => break,
            }
        }
        let device = self.device.clone();
        let s = self.surface(surface);
        let Some((format, usage, _)) = s.asked else {
            panic!("[gpu] surface {surface} drawn before it is configured")
        };
        let usage = usage | wgpu::TextureUsages::RENDER_ATTACHMENT;
        let size = wgpu::Extent3d {
            width: s.size[0],
            height: s.size[1],
            depth_or_array_layers: 1,
        };
        let kept = s.unshown.take().filter(|t| t.format() == format && t.usage() == usage && t.size() == size);
        let texture = kept.unwrap_or_else(|| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        });
        s.unshown = Some(texture.clone());
        Frame::Unshown(texture)
    }

    fn surface(&mut self, surface: u32) -> &mut Surface {
        let at = surface as usize;
        if self.surfaces.len() <= at {
            self.surfaces.resize_with(at + 1, Surface::default);
        }
        &mut self.surfaces[at]
    }

    /// `surface`'s configuration made stale, and configured where it may be.
    fn want_configured(&mut self, surface: u32) {
        let s = self.surface(surface);
        let Some(b) = s.bound.as_mut().filter(|_| s.asked.is_some()) else { return };
        b.stale = true;
        s.frame = None;
        match self.configure_at {
            ConfigureAt::Here => self.configure_surfaces(),
            ConfigureAt::Elsewhere(ask) => ask(),
        }
    }

    /// The device went: every object with it, and the guest told in its block.
    fn lost(&mut self, memory: &mut Memory<'_>) {
        self.objects.clear();
        self.pending.clear();
        self.decoding.clear();
        if let Some((address, words)) = self.block {
            memory.set_word(address + 4 * (words - 1), STATUS::LOST);
        }
    }

    /// What the device has answered since: its loss, which ends everything
    /// else; else each mapping's bytes copied to its address, its own word set
    /// and the buffer unmapped; each done's word set where its ticket still
    /// stands.
    /// Run at the turn's edge, before the turn reads its words.
    pub fn poll(&mut self, memory: &mut Memory<'_>) {
        if self.gone.swap(false, Ordering::AcqRel) {
            self.lost(memory);
            return;
        }
        let _ = self.device.poll(wgpu::PollType::Poll);
        self.pending.retain(|p| match p.state.load(Ordering::Acquire) {
            WAITING => true,
            ANSWERED => {
                if let Some((buffer, offset, size, address)) = &p.read {
                    let slice = buffer.slice(*offset..*offset + *size);
                    if let Ok(range) = slice.get_mapped_range() {
                        memory.write(*address as usize, &range);
                    }
                    buffer.unmap();
                    memory.set_word(p.word, p.ticket | DONE);
                } else if memory.word(p.word) == p.ticket {
                    memory.set_word(p.word, p.ticket | DONE);
                }
                false
            }
            _ => false,
        });
        while let Ok((job, image)) = self.decoder.answers.try_recv() {
            // A job the device's loss forgot answers to nothing.
            let Some(d) = self.decoding.pop_front_if(|d| d.job == job) else { continue };
            // An image the host cannot decode is left out and its word not
            // written, as the web leaves out one its browser cannot; one whose
            // texture went meanwhile has nowhere to land.
            let held = self.objects.iter().flatten().any(|o| matches!(o, Object::Texture(t) if *t == d.to.texture));
            let Some(image) = image.filter(|_| held) else { continue };
            let layout = wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * image.width),
                rows_per_image: Some(image.height),
            };
            let size = wgpu::Extent3d {
                width: image.width,
                height: image.height,
                depth_or_array_layers: 1,
            };
            self.queue.write_texture(d.to.info(), &image.rgba, layout, size);
            // In the queue ahead of anything the guest submits from here.
            if memory.word(d.word) == d.ticket {
                memory.set_word(d.word, d.ticket | DONE);
            }
        }
    }

    /// Whether this turn may draw a frame: while fewer than `FRAME_LATENCY`
    /// admitted frames are unfinished, so each surface's next texture is free
    /// and taking it does not block. Asked once a turn, before the turn draws;
    /// on the web the browser paces frames and every one is admitted.
    pub fn admit(&mut self) -> bool {
        self.admitted = UNFINISHED.load(Ordering::Relaxed) < FRAME_LATENCY;
        self.admitted
    }

    /// Each surface's texture this turn drew, shown. An admitted frame is
    /// unfinished from here until the queue has done the work submitted with
    /// it.
    pub fn present(&mut self) {
        for s in &mut self.surfaces {
            if let Some(Frame::Shown(frame)) = s.frame.take() {
                self.queue.present(frame);
            }
        }
        if core::mem::take(&mut self.admitted) {
            UNFINISHED.fetch_add(1, Ordering::Relaxed);
            self.queue.on_submitted_work_done(|| {
                UNFINISHED.fetch_sub(1, Ordering::Relaxed);
            });
        }
    }

    /// Runs the stream at `at`, `head` words of head, as the turn left it:
    /// word 0 of the head the count of record words past it, each record its
    /// op, its count of words, then they.
    pub fn run(&mut self, memory: &mut Memory<'_>, at: u32, head: u32) {
        let end = memory.word(at);
        let first = at + 4 * head;
        let mut k = 0;
        while k + 2 <= end {
            let code = memory.word(first + 4 * k);
            let n = memory.word(first + 4 * (k + 1));
            let mut scratch = core::mem::take(&mut self.scratch);
            scratch.clear();
            let from = (first + 4 * (k + 2)) as usize;
            scratch.extend(memory.read(from, 4 * n as usize).as_chunks::<4>().0.iter().map(|w| u32::from_le_bytes(*w)));
            self.record(memory, code, &mut Words { w: &scratch, at: 0 });
            self.scratch = scratch;
            k += 2 + n;
        }
        // The turn's writes reach the queue with it, as the web's writeBuffer does.
        if self.uploads.is_some() {
            self.submit(core::iter::empty());
        }
    }

    /// `bytes` into buffer `id` at `offset` through the belt; nothing for none,
    /// as WebGPU writes nothing.
    fn write(&mut self, id: u32, offset: u64, bytes: &[u8]) {
        let Some(size) = core::num::NonZeroU64::new(bytes.len() as u64) else { return };
        let buffer = self.buffer(id).clone();
        let encoder = self.uploads.get_or_insert_with(|| self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None }));
        self.belt.write_buffer(encoder, &buffer, offset, size).copy_from_slice(bytes);
    }

    /// `then` submitted after the writes made since the last submit.
    fn submit(&mut self, then: impl IntoIterator<Item = wgpu::CommandBuffer>) {
        let uploads = self.uploads.take().map(|e| {
            self.belt.finish();
            e.finish()
        });
        let staged = uploads.is_some();
        self.queue.submit(uploads.into_iter().chain(then));
        if staged {
            self.belt.recall();
        }
    }

    fn put(&mut self, id: u32, o: Object) {
        let slot = handle_slot(id) as usize;
        if self.objects.len() <= slot {
            self.objects.resize_with(slot + 1, || None);
        }
        self.objects[slot] = Some(o);
    }

    fn get(&self, id: u32) -> &Object {
        self.objects.get(handle_slot(id) as usize).and_then(Option::as_ref).unwrap_or_else(|| panic!("[gpu] no object {id}"))
    }

    fn get_mut(&mut self, id: u32) -> &mut Object {
        self.objects
            .get_mut(handle_slot(id) as usize)
            .and_then(Option::as_mut)
            .unwrap_or_else(|| panic!("[gpu] no object {id}"))
    }

    fn take(&mut self, id: u32) -> Option<Object> {
        self.objects.get_mut(handle_slot(id) as usize).and_then(Option::take)
    }

    fn buffer(&self, id: u32) -> &wgpu::Buffer {
        match self.get(id) {
            Object::Buffer(b) => b,
            _ => panic!("[gpu] {id} is no buffer"),
        }
    }

    fn texture(&self, id: u32) -> &wgpu::Texture {
        match self.get(id) {
            Object::Texture(t) => t,
            _ => panic!("[gpu] {id} is no texture"),
        }
    }

    fn view(&self, id: u32) -> (&wgpu::TextureView, wgpu::TextureFormat) {
        match self.get(id) {
            Object::View(v, format) => (v, *format),
            _ => panic!("[gpu] {id} is no view"),
        }
    }

    fn query_set(&self, id: u32) -> &wgpu::QuerySet {
        match self.get(id) {
            Object::QuerySet(q) => q,
            _ => panic!("[gpu] {id} is no query set"),
        }
    }

    fn shader(&self, id: u32) -> &wgpu::ShaderModule {
        match self.get(id) {
            Object::Shader(s) => s,
            _ => panic!("[gpu] {id} is no shader module"),
        }
    }

    fn encoder(&mut self, id: u32) -> &mut wgpu::CommandEncoder {
        match self.get_mut(id) {
            Object::Encoder(e) => e,
            _ => panic!("[gpu] {id} is no encoder"),
        }
    }

    fn render_pass(&mut self, id: u32) -> &mut wgpu::RenderPass<'static> {
        match self.get_mut(id) {
            Object::RenderPass(p) => p,
            _ => panic!("[gpu] {id} is no render pass"),
        }
    }

    fn compute_pass(&mut self, id: u32) -> &mut wgpu::ComputePass<'static> {
        match self.get_mut(id) {
            Object::ComputePass(p) => p,
            _ => panic!("[gpu] {id} is no compute pass"),
        }
    }

    fn texture_place(&self, r: &mut Words) -> OwnedTexture {
        let texture = self.texture(r.u()).clone();
        let mip_level = r.u();
        let origin = wgpu::Origin3d { x: r.u(), y: r.u(), z: r.u() };
        let aspect = of(&self.names.aspect, r.u(), "aspect");
        OwnedTexture { texture, mip_level, origin, aspect }
    }

    fn buffer_layout(&self, r: &mut Words) -> OwnedBuffer {
        let buffer = self.buffer(r.u()).clone();
        let offset = u64::from(r.u());
        let layout = wgpu::TexelCopyBufferLayout {
            offset,
            bytes_per_row: Some(r.u()),
            rows_per_image: Some(r.u()),
        };
        OwnedBuffer { buffer, layout }
    }

    fn extent(r: &mut Words) -> wgpu::Extent3d {
        wgpu::Extent3d {
            width: r.u(),
            height: r.u(),
            depth_or_array_layers: r.u(),
        }
    }

    /// A pass's timestamp writes: its query set and the indices it writes,
    /// none where the guest sets no query set.
    fn timestamps(&self, r: &mut Words) -> Option<(wgpu::QuerySet, Option<u32>, Option<u32>)> {
        let (set, begin, end) = (r.opt(), r.opt(), r.opt());
        set.map(|s| (self.query_set(s).clone(), begin, end))
    }

    fn stencil_face(&self, r: &mut Words) -> wgpu::StencilFaceState {
        wgpu::StencilFaceState {
            compare: of(&self.names.compare, r.u(), "compare"),
            fail_op: of(&self.names.stencil, r.u(), "stencil"),
            depth_fail_op: of(&self.names.stencil, r.u(), "stencil"),
            pass_op: of(&self.names.stencil, r.u(), "stencil"),
        }
    }

    fn record(&mut self, memory: &mut Memory<'_>, code: u32, r: &mut Words) {
        match code {
            op::BUFFER => {
                let id = r.u();
                let (size, usage, mapped) = (u64::from(r.u()), r.u(), r.flag());
                let b = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: None,
                    size,
                    usage: flags(&BUFFER_USAGES, wgpu::BufferUsages::empty(), usage),
                    mapped_at_creation: mapped,
                });
                self.put(id, Object::Buffer(b));
            }
            op::TEXTURE => {
                let id = r.u();
                let size = Self::extent(r);
                let (mip_level_count, sample_count) = (r.u(), r.u());
                let dimension = of(&self.names.dimension, r.u(), "dimension");
                let format = of(&self.names.texture_format, r.u(), "format");
                let usage = flags(&TEXTURE_USAGES, wgpu::TextureUsages::empty(), r.u());
                let view_formats: Vec<wgpu::TextureFormat> = (0..r.u()).map(|_| of(&self.names.texture_format, r.u(), "format")).collect();
                let t = self.device.create_texture(&wgpu::TextureDescriptor {
                    label: None,
                    size,
                    mip_level_count,
                    sample_count,
                    dimension,
                    format,
                    usage,
                    view_formats: &view_formats,
                });
                self.put(id, Object::Texture(t));
            }
            op::VIEW => {
                let id = r.u();
                let texture = self.texture(r.u()).clone();
                let format = r.opt().map(|f| of(&self.names.texture_format, f, "format"));
                let dimension = r.opt().map(|v| of(&self.names.view_dimension, v, "view dimension"));
                let aspect = of(&self.names.aspect, r.u(), "aspect");
                let (base_mip_level, mip_level_count, base_array_layer, array_layer_count) = (r.u(), r.opt(), r.u(), r.opt());
                // The rest as WebGPU defaults them, whatever fields wgpu adds.
                let v = texture.create_view(&wgpu::TextureViewDescriptor {
                    format,
                    dimension,
                    aspect,
                    base_mip_level,
                    mip_level_count,
                    base_array_layer,
                    array_layer_count,
                    ..Default::default()
                });
                self.put(id, Object::View(v, format.unwrap_or(texture.format())));
            }
            op::SAMPLER => {
                let id = r.u();
                let address = [r.u(), r.u(), r.u()].map(|a| of(&self.names.address, a, "address mode"));
                let (mag, min) = (of(&self.names.filter, r.u(), "filter"), of(&self.names.filter, r.u(), "filter"));
                let mip = of(&self.names.mipmap, r.u(), "mipmap filter");
                let (lod_min_clamp, lod_max_clamp) = (r.f32(), r.f32());
                let compare = r.opt().map(|c| of(&self.names.compare, c, "compare"));
                let anisotropy_clamp = r.u() as u16;
                let s = self.device.create_sampler(&wgpu::SamplerDescriptor {
                    label: None,
                    address_mode_u: address[0],
                    address_mode_v: address[1],
                    address_mode_w: address[2],
                    mag_filter: mag,
                    min_filter: min,
                    mipmap_filter: mip,
                    lod_min_clamp,
                    lod_max_clamp,
                    compare,
                    anisotropy_clamp,
                    border_color: None,
                });
                self.put(id, Object::Sampler(s));
            }
            op::SHADER_MODULE => {
                let id = r.u();
                let code = r.text();
                let m = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: None,
                    source: wgpu::ShaderSource::Wgsl(code.into()),
                });
                self.put(id, Object::Shader(m));
            }
            op::BIND_GROUP_LAYOUT => {
                let id = r.u();
                let entries: Vec<wgpu::BindGroupLayoutEntry> = (0..r.u())
                    .map(|_| {
                        let (binding, visibility, kind, a, b, c) = (r.u(), r.u(), r.u(), r.u(), r.u(), r.u());
                        let ty = match kind {
                            0 => wgpu::BindingType::Buffer {
                                ty: match BUFFER_BINDING_TYPE.get(a as usize).copied() {
                                    Some("uniform") => wgpu::BufferBindingType::Uniform,
                                    Some("storage") => wgpu::BufferBindingType::Storage { read_only: false },
                                    Some("read-only-storage") => wgpu::BufferBindingType::Storage { read_only: true },
                                    _ => panic!("[gpu] buffer binding type {a}"),
                                },
                                has_dynamic_offset: b != 0,
                                min_binding_size: core::num::NonZeroU64::new(u64::from(c)),
                            },
                            1 => wgpu::BindingType::Sampler(of(&self.names.sampler_binding, a, "sampler binding type")),
                            2 => wgpu::BindingType::Texture {
                                sample_type: match TEXTURE_SAMPLE_TYPE.get(a as usize).copied() {
                                    Some("float") => wgpu::TextureSampleType::Float { filterable: true },
                                    Some("unfilterable-float") => wgpu::TextureSampleType::Float { filterable: false },
                                    Some("depth") => wgpu::TextureSampleType::Depth,
                                    Some("sint") => wgpu::TextureSampleType::Sint,
                                    Some("uint") => wgpu::TextureSampleType::Uint,
                                    _ => panic!("[gpu] texture sample type {a}"),
                                },
                                view_dimension: of(&self.names.view_dimension, b, "view dimension"),
                                multisampled: c != 0,
                            },
                            _ => wgpu::BindingType::StorageTexture {
                                access: of(&self.names.storage_access, a, "storage access"),
                                format: of(&self.names.texture_format, b, "format"),
                                view_dimension: of(&self.names.view_dimension, c, "view dimension"),
                            },
                        };
                        wgpu::BindGroupLayoutEntry {
                            binding,
                            visibility: flags(&SHADER_STAGES, wgpu::ShaderStages::empty(), visibility),
                            ty,
                            count: None,
                        }
                    })
                    .collect();
                let l = self.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: None, entries: &entries });
                self.put(id, Object::BindGroupLayout(l));
            }
            op::PIPELINE_LAYOUT => {
                let id = r.u();
                let ids: Vec<u32> = (0..r.u()).map(|_| r.u()).collect();
                let layouts: Vec<Option<&wgpu::BindGroupLayout>> = ids
                    .iter()
                    .map(|&l| match self.get(l) {
                        Object::BindGroupLayout(l) => Some(l),
                        _ => panic!("[gpu] {l} is no bind group layout"),
                    })
                    .collect();
                let p = self.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: None,
                    bind_group_layouts: &layouts,
                    immediate_size: 0,
                });
                self.put(id, Object::PipelineLayout(p));
            }
            op::BIND_GROUP => {
                let id = r.u();
                let layout = r.u();
                let words: Vec<[u32; 5]> = (0..r.u()).map(|_| [r.u(), r.u(), r.u(), r.u(), r.u()]).collect();
                let entries: Vec<wgpu::BindGroupEntry> = words
                    .iter()
                    .map(|&[binding, kind, object, offset, size]| {
                        let resource = match (kind, self.get(object)) {
                            (0, Object::Buffer(b)) => wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                buffer: b,
                                offset: u64::from(offset),
                                size: Some(size).filter(|&s| s != NONE).and_then(|s| core::num::NonZeroU64::new(u64::from(s))),
                            }),
                            (_, Object::Sampler(s)) => wgpu::BindingResource::Sampler(s),
                            (_, Object::View(v, _)) => wgpu::BindingResource::TextureView(v),
                            _ => panic!("[gpu] binding {binding}'s object {object} is no resource"),
                        };
                        wgpu::BindGroupEntry { binding, resource }
                    })
                    .collect();
                let layout = match self.get(layout) {
                    Object::BindGroupLayout(l) => l,
                    _ => panic!("[gpu] {layout} is no bind group layout"),
                };
                let g = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    layout,
                    entries: &entries,
                });
                self.put(id, Object::BindGroup(g));
            }
            op::RENDER_PIPELINE => self.render_pipeline(r),
            op::COMPUTE_PIPELINE => {
                let id = r.u();
                let layout = r.opt();
                let module = r.u();
                let entry = r.text();
                let layout = layout.map(|l| match self.get(l) {
                    Object::PipelineLayout(l) => l,
                    _ => panic!("[gpu] {l} is no pipeline layout"),
                });
                let p = self.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: None,
                    layout,
                    module: self.shader(module),
                    entry_point: Some(&entry),
                    compilation_options: Default::default(),
                    cache: None,
                });
                self.put(id, Object::ComputePipeline(p));
            }
            op::QUERY_SET => {
                let id = r.u();
                let ty = match QUERY_TYPE.get(r.u() as usize).copied() {
                    Some("occlusion") => wgpu::QueryType::Occlusion,
                    Some("timestamp") => wgpu::QueryType::Timestamp,
                    t => panic!("[gpu] query type {t:?}"),
                };
                let q = self.device.create_query_set(&wgpu::QuerySetDescriptor { label: None, ty, count: r.u() });
                self.put(id, Object::QuerySet(q));
            }
            op::DESTROY => {
                let id = r.u();
                match self.take(id) {
                    Some(Object::Buffer(b)) => b.destroy(),
                    Some(Object::Texture(t)) => t.destroy(),
                    _ => {}
                }
            }
            op::WRITE_BUFFER => {
                let (id, offset) = (r.u(), u64::from(r.u()));
                let bytes = r.bytes();
                self.write(id, offset, bytes);
            }
            op::WRITE_BUFFER_AT => {
                let (id, offset, address, size) = (r.u(), u64::from(r.u()), r.u() as usize, r.u() as usize);
                self.write(id, offset, memory.read(address, size));
            }
            op::WRITE_TEXTURE | op::WRITE_TEXTURE_AT => {
                let place = self.texture_place(r);
                let layout = wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(r.u()),
                    rows_per_image: Some(r.u()),
                };
                let size = Self::extent(r);
                if code == op::WRITE_TEXTURE {
                    let bytes = r.bytes();
                    self.queue.write_texture(place.info(), bytes, layout, size);
                } else {
                    let (address, n) = (r.u() as usize, r.u() as usize);
                    self.queue.write_texture(place.info(), memory.read(address, n), layout, size);
                }
            }
            op::COPY_EXTERNAL_IMAGE => {
                let to = self.texture_place(r);
                let (flip_y, premultiplied) = (r.flag(), r.flag());
                let (address, n, word, ticket) = (r.u() as usize, r.u() as usize, r.u(), r.u());
                // Copied now: the guest's bytes are only its until the next
                // turn; decoded off the turn, so a large image costs none.
                let job = self.decoder.next;
                self.decoder.next += 1;
                let _ = self.decoder.ask.send((job, memory.read(address, n).to_vec(), flip_y, premultiplied));
                self.decoding.push_back(Decoding { job, to, word, ticket });
            }
            op::ENCODER => {
                let id = r.u();
                let e = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
                self.put(id, Object::Encoder(e));
            }
            op::COPY_BUFFER_TO_BUFFER => {
                let (e, from, from_at, to, to_at, size) = (r.u(), r.u(), u64::from(r.u()), r.u(), u64::from(r.u()), u64::from(r.u()));
                let (from, to) = (self.buffer(from).clone(), self.buffer(to).clone());
                self.encoder(e).copy_buffer_to_buffer(&from, from_at, &to, to_at, size);
            }
            op::COPY_BUFFER_TO_TEXTURE => {
                let e = r.u();
                let (from, to) = (self.buffer_layout(r), self.texture_place(r));
                let size = Self::extent(r);
                self.encoder(e).copy_buffer_to_texture(from.info(), to.info(), size);
            }
            op::COPY_TEXTURE_TO_BUFFER => {
                let e = r.u();
                let (from, to) = (self.texture_place(r), self.buffer_layout(r));
                let size = Self::extent(r);
                self.encoder(e).copy_texture_to_buffer(from.info(), to.info(), size);
            }
            op::COPY_TEXTURE_TO_TEXTURE => {
                let e = r.u();
                let (from, to) = (self.texture_place(r), self.texture_place(r));
                let size = Self::extent(r);
                self.encoder(e).copy_texture_to_texture(from.info(), to.info(), size);
            }
            op::CLEAR_BUFFER => {
                let (e, b, offset, size) = (r.u(), r.u(), u64::from(r.u()), r.opt().map(u64::from));
                let b = self.buffer(b).clone();
                self.encoder(e).clear_buffer(&b, offset, size);
            }
            op::RESOLVE_QUERY_SET => {
                let (e, q, first, count, b, offset) = (r.u(), r.u(), r.u(), r.u(), r.u(), u64::from(r.u()));
                let (q, b) = (self.query_set(q).clone(), self.buffer(b).clone());
                self.encoder(e).resolve_query_set(&q, first..first + count, &b, offset);
            }
            op::RENDER_PASS => self.begin_render_pass(r),
            op::COMPUTE_PASS => {
                let (id, e) = (r.u(), r.u());
                let stamps = self.timestamps(r);
                let timestamp_writes = stamps.as_ref().map(|(q, begin, end)| wgpu::ComputePassTimestampWrites {
                    query_set: q,
                    beginning_of_pass_write_index: *begin,
                    end_of_pass_write_index: *end,
                });
                let p = self.encoder(e).begin_compute_pass(&wgpu::ComputePassDescriptor { label: None, timestamp_writes }).forget_lifetime();
                self.put(id, Object::ComputePass(Box::new(p)));
            }
            op::SET_PIPELINE => {
                let (pass, pipeline) = (r.u(), r.u());
                match self.get(pipeline) {
                    Object::RenderPipeline(p) => {
                        let p = p.clone();
                        self.render_pass(pass).set_pipeline(&p);
                    }
                    Object::ComputePipeline(p) => {
                        let p = p.clone();
                        self.compute_pass(pass).set_pipeline(&p);
                    }
                    _ => panic!("[gpu] {pipeline} is no pipeline"),
                }
            }
            op::SET_BIND_GROUP => {
                let (pass, index, group, offsets) = (r.u(), r.u(), r.u(), r.list());
                let group = match self.get(group) {
                    Object::BindGroup(g) => g.clone(),
                    _ => panic!("[gpu] {group} is no bind group"),
                };
                match self.get_mut(pass) {
                    Object::RenderPass(p) => p.set_bind_group(index, &group, offsets),
                    Object::ComputePass(p) => p.set_bind_group(index, &group, offsets),
                    _ => panic!("[gpu] {pass} is no pass"),
                }
            }
            op::SET_VERTEX_BUFFER => {
                let (pass, slot, b, offset, size) = (r.u(), r.u(), r.u(), u64::from(r.u()), r.opt().map(u64::from));
                let b = self.buffer(b).clone();
                self.render_pass(pass).set_vertex_buffer(slot, slice(&b, offset, size));
            }
            op::SET_INDEX_BUFFER => {
                let (pass, b) = (r.u(), r.u());
                let format = of(&self.names.index_format, r.u(), "index format");
                let (offset, size) = (u64::from(r.u()), r.opt().map(u64::from));
                let b = self.buffer(b).clone();
                self.render_pass(pass).set_index_buffer(slice(&b, offset, size), format);
            }
            op::SET_VIEWPORT => {
                let pass = r.u();
                let v = [r.f32(), r.f32(), r.f32(), r.f32(), r.f32(), r.f32()];
                self.render_pass(pass).set_viewport(v[0], v[1], v[2], v[3], v[4], v[5]);
            }
            op::SET_SCISSOR => {
                let pass = r.u();
                let s = [r.u(), r.u(), r.u(), r.u()];
                self.render_pass(pass).set_scissor_rect(s[0], s[1], s[2], s[3]);
            }
            op::SET_BLEND_CONSTANT => {
                let pass = r.u();
                let c = [r.f32(), r.f32(), r.f32(), r.f32()].map(f64::from);
                self.render_pass(pass).set_blend_constant(wgpu::Color { r: c[0], g: c[1], b: c[2], a: c[3] });
            }
            op::SET_STENCIL_REFERENCE => {
                let pass = r.u();
                let reference = r.u();
                self.render_pass(pass).set_stencil_reference(reference);
            }
            op::DRAW => {
                let (pass, vertices, instances, first_vertex, first_instance) = (r.u(), r.u(), r.u(), r.u(), r.u());
                self.render_pass(pass).draw(first_vertex..first_vertex + vertices, first_instance..first_instance + instances);
            }
            op::DRAW_INDEXED => {
                let (pass, indices, instances, first_index, base_vertex, first_instance) = (r.u(), r.u(), r.u(), r.u(), r.i(), r.u());
                self.render_pass(pass)
                    .draw_indexed(first_index..first_index + indices, base_vertex, first_instance..first_instance + instances);
            }
            op::DRAW_INDIRECT | op::DRAW_INDEXED_INDIRECT => {
                let (pass, b, offset) = (r.u(), r.u(), u64::from(r.u()));
                let b = self.buffer(b).clone();
                let p = self.render_pass(pass);
                if code == op::DRAW_INDIRECT {
                    p.draw_indirect(&b, offset);
                } else {
                    p.draw_indexed_indirect(&b, offset);
                }
            }
            op::DISPATCH => {
                let (pass, x, y, z) = (r.u(), r.u(), r.u(), r.u());
                self.compute_pass(pass).dispatch_workgroups(x, y, z);
            }
            op::DISPATCH_INDIRECT => {
                let (pass, b, offset) = (r.u(), r.u(), u64::from(r.u()));
                let b = self.buffer(b).clone();
                self.compute_pass(pass).dispatch_workgroups_indirect(&b, offset);
            }
            op::END => {
                let id = r.u();
                drop(self.take(id));
            }
            op::SUBMIT => {
                let mut finished = core::mem::take(&mut self.finished);
                finished.extend(r.list().iter().filter_map(|&id| match self.take(id) {
                    Some(Object::Encoder(e)) => Some(e.finish()),
                    _ => None,
                }));
                self.submit(finished.drain(..));
                self.finished = finished;
            }
            op::MAP_ASYNC => {
                let (b, offset, size, address, word, ticket) = (r.u(), u64::from(r.u()), u64::from(r.u()), r.u(), r.u(), r.u());
                let buffer = self.buffer(b).clone();
                let state = Arc::new(AtomicU8::new(WAITING));
                let answered = Arc::clone(&state);
                buffer.slice(offset..offset + size).map_async(wgpu::MapMode::Read, move |result| {
                    answered.store(if result.is_ok() { ANSWERED } else { REFUSED }, Ordering::Release);
                });
                self.pending.push(Pending {
                    state,
                    word,
                    ticket,
                    read: Some((buffer, offset, size, address)),
                });
            }
            op::QUEUE_DONE => {
                let (word, ticket) = (r.u(), r.u());
                // What was written is the queue's work as much as what was submitted.
                self.submit(core::iter::empty());
                let state = Arc::new(AtomicU8::new(WAITING));
                let answered = Arc::clone(&state);
                self.queue.on_submitted_work_done(move || answered.store(ANSWERED, Ordering::Release));
                self.pending.push(Pending { state, word, ticket, read: None });
            }
            op::CONFIGURE => {
                let (surface, format, usage, alpha) = (r.u(), r.u(), flags(&TEXTURE_USAGES, wgpu::TextureUsages::empty(), r.u()), r.u());
                let format = if format == SURFACE_PREFERRED {
                    self.preferred
                } else {
                    of(&self.names.texture_format, format, "format")
                };
                let premultiplied = CANVAS_ALPHA_MODE.get(alpha as usize) == Some(&"premultiplied");
                self.surface(surface).asked = Some((format, usage, premultiplied));
                self.want_configured(surface);
            }
            op::RESIZE => {
                let (surface, width, height) = (r.u(), r.u(), r.u());
                let size = [width.max(1), height.max(1)];
                let s = self.surface(surface);
                if s.size != size {
                    s.size = size;
                    self.want_configured(surface);
                }
            }
            op::CURRENT => {
                let (id, surface) = (r.u(), r.u());
                let frame = match self.surface(surface).frame.take() {
                    Some(frame) => frame,
                    None => self.current(surface),
                };
                let texture = frame.texture().clone();
                self.surface(surface).frame = Some(frame);
                let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
                self.put(id, Object::View(view, texture.format()));
            }
            op::DEVICE => {
                let (address, words) = (r.u(), r.u());
                self.block = Some((address, words));
                let (limits, features) = (self.device.limits(), self.device.features());
                for (k, name) in LIMIT_NAME.iter().enumerate() {
                    memory.set_word(address + 4 * k as u32, limit(&limits, name));
                }
                let mut bits = vec![0u32; FEATURE_NAME.len().div_ceil(32)];
                for (k, name) in FEATURE_NAME.iter().enumerate() {
                    if feature(name).is_some_and(|f| features.contains(f)) {
                        bits[k >> 5] |= 1 << (k & 31);
                    }
                }
                let period_at = LIMIT_NAME.len() + bits.len();
                for (k, w) in bits.into_iter().enumerate() {
                    memory.set_word(address + 4 * (LIMIT_NAME.len() + k) as u32, w);
                }
                // wgpu resolves timestamps in ticks of this many nanoseconds.
                memory.set_word(address + 4 * period_at as u32, self.queue.get_timestamp_period().to_bits());
                let preferred = self.names.texture_format.iter().position(|f| *f == Some(self.preferred));
                memory.set_word(
                    address + 4 * (period_at + 1) as u32,
                    preferred.unwrap_or_else(|| panic!("[gpu] {:?} is no format the spec names", self.preferred)) as u32,
                );
                memory.set_word(address + 4 * (words - 1), STATUS::READY);
            }
            _ => panic!("[gpu] op {code} is no op of spec/gpu/api.json"),
        }
    }

    fn render_pipeline(&mut self, r: &mut Words) {
        let id = r.u();
        let layout = r.opt();
        let vertex_module = r.u();
        let vertex_entry = r.text();
        struct Layout {
            stride: u64,
            step: wgpu::VertexStepMode,
            attributes: Vec<wgpu::VertexAttribute>,
        }
        let buffers: Vec<Layout> = (0..r.u())
            .map(|_| {
                let stride = u64::from(r.u());
                let step = of(&self.names.step, r.u(), "step mode");
                let attributes = (0..r.u())
                    .map(|_| wgpu::VertexAttribute {
                        format: of(&self.names.vertex_format, r.u(), "vertex format"),
                        offset: u64::from(r.u()),
                        shader_location: r.u(),
                    })
                    .collect();
                Layout { stride, step, attributes }
            })
            .collect();
        let topology = of(&self.names.topology, r.u(), "topology");
        let strip_index_format = r.opt().map(|s| of(&self.names.index_format, s, "index format"));
        let front_face = of(&self.names.front_face, r.u(), "front face");
        let cull_mode = match CULL_MODE.get(r.u() as usize).copied() {
            Some("front") => Some(wgpu::Face::Front),
            Some("back") => Some(wgpu::Face::Back),
            _ => None,
        };
        let unclipped_depth = r.flag();
        let depth_format = r.opt();
        let depth_write_enabled = r.flag();
        let depth_compare = of(&self.names.compare, r.u(), "compare");
        let (front, back) = (self.stencil_face(r), self.stencil_face(r));
        let (read_mask, write_mask) = (r.u(), r.u());
        let bias = wgpu::DepthBiasState {
            constant: r.i(),
            slope_scale: r.f32(),
            clamp: r.f32(),
        };
        let multisample = wgpu::MultisampleState {
            count: r.u(),
            mask: u64::from(r.u()),
            alpha_to_coverage_enabled: r.flag(),
        };
        let fragment_module = r.opt();
        let fragment_entry = r.text();
        let targets: Vec<Option<wgpu::ColorTargetState>> = (0..r.u())
            .map(|_| {
                let format = of(&self.names.texture_format, r.u(), "format");
                let (cop, csrc, cdst, aop, asrc, adst) = (r.u(), r.u(), r.u(), r.u(), r.u(), r.u());
                let component = |op: u32, src: u32, dst: u32| wgpu::BlendComponent {
                    operation: of(&self.names.blend_operation, op, "blend operation"),
                    src_factor: of(&self.names.blend_factor, src, "blend factor"),
                    dst_factor: of(&self.names.blend_factor, dst, "blend factor"),
                };
                let blend = (cop != NONE).then(|| wgpu::BlendState {
                    color: component(cop, csrc, cdst),
                    alpha: component(aop, asrc, adst),
                });
                Some(wgpu::ColorTargetState {
                    format,
                    blend,
                    write_mask: flags(&COLOR_WRITES, wgpu::ColorWrites::empty(), r.u()),
                })
            })
            .collect();
        let vertex_buffers: Vec<Option<wgpu::VertexBufferLayout>> = buffers
            .iter()
            .map(|b| {
                Some(wgpu::VertexBufferLayout {
                    array_stride: b.stride,
                    step_mode: b.step,
                    attributes: &b.attributes,
                })
            })
            .collect();
        let layout = layout.map(|l| match self.get(l) {
            Object::PipelineLayout(l) => l,
            _ => panic!("[gpu] {l} is no pipeline layout"),
        });
        let p = self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout,
            vertex: wgpu::VertexState {
                module: self.shader(vertex_module),
                entry_point: Some(&vertex_entry),
                compilation_options: Default::default(),
                buffers: &vertex_buffers,
            },
            primitive: wgpu::PrimitiveState {
                topology,
                strip_index_format,
                front_face,
                cull_mode,
                unclipped_depth,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: depth_format.map(|f| wgpu::DepthStencilState {
                format: of(&self.names.texture_format, f, "format"),
                depth_write_enabled: Some(depth_write_enabled),
                depth_compare: Some(depth_compare),
                stencil: wgpu::StencilState { front, back, read_mask, write_mask },
                bias,
            }),
            multisample,
            fragment: fragment_module.map(|m| wgpu::FragmentState {
                module: self.shader(m),
                entry_point: Some(&fragment_entry),
                compilation_options: Default::default(),
                targets: &targets,
            }),
            multiview_mask: None,
            cache: None,
        });
        self.put(id, Object::RenderPipeline(p));
    }

    fn begin_render_pass(&mut self, r: &mut Words) {
        let (id, e) = (r.u(), r.u());
        struct Color {
            view: wgpu::TextureView,
            resolve: Option<wgpu::TextureView>,
            ops: wgpu::Operations<wgpu::Color>,
        }
        let n = r.u() as usize;
        if n > COLORS {
            panic!("[gpu] a pass of {n} color targets");
        }
        let mut colors: [Option<Color>; COLORS] = core::array::from_fn(|_| None);
        for color in &mut colors[..n] {
            let view = self.view(r.u()).0.clone();
            let resolve = r.opt().map(|v| self.view(v).0.clone());
            let (load, store) = (r.u(), r.u());
            let c = [r.f32(), r.f32(), r.f32(), r.f32()].map(f64::from);
            let clear = wgpu::Color { r: c[0], g: c[1], b: c[2], a: c[3] };
            *color = Some(Color {
                view,
                resolve,
                ops: wgpu::Operations {
                    load: load_op(load, clear),
                    store: store_op(store),
                },
            });
        }
        let depth = r.opt();
        let (dload, dstore, dclear, dro, sload, sstore, sclear, sro) = (r.u(), r.u(), r.f32(), r.flag(), r.u(), r.u(), r.u(), r.flag());
        // WebGPU refuses ops on an aspect the format lacks, or one read only.
        let depth = depth.map(|v| {
            let (view, format) = self.view(v);
            let (view, has_depth, has_stencil) = (view.clone(), format.has_depth_aspect(), format.has_stencil_aspect());
            let depth_ops = (has_depth && !dro).then(|| wgpu::Operations {
                load: load_op(dload, dclear),
                store: store_op(dstore),
            });
            let stencil_ops = (has_stencil && !sro).then(|| wgpu::Operations {
                load: load_op(sload, sclear),
                store: store_op(sstore),
            });
            (view, depth_ops, stencil_ops)
        });
        let stamps = self.timestamps(r);
        let occlusion = r.opt().map(|q| self.query_set(q).clone());
        let color_attachments: [Option<wgpu::RenderPassColorAttachment>; COLORS] = core::array::from_fn(|k| {
            colors[k].as_ref().map(|c| wgpu::RenderPassColorAttachment {
                view: &c.view,
                depth_slice: None,
                resolve_target: c.resolve.as_ref(),
                ops: c.ops,
            })
        });
        let depth_stencil_attachment = depth.as_ref().map(|(view, depth_ops, stencil_ops)| wgpu::RenderPassDepthStencilAttachment {
            view,
            depth_ops: *depth_ops,
            stencil_ops: *stencil_ops,
        });
        let timestamp_writes = stamps.as_ref().map(|(q, begin, end)| wgpu::RenderPassTimestampWrites {
            query_set: q,
            beginning_of_pass_write_index: *begin,
            end_of_pass_write_index: *end,
        });
        let p = self
            .encoder(e)
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &color_attachments[..n],
                depth_stencil_attachment,
                timestamp_writes,
                occlusion_query_set: occlusion.as_ref(),
                multiview_mask: None,
            })
            .forget_lifetime();
        self.put(id, Object::RenderPass(Box::new(p)));
    }
}

/// The most color targets a render pass takes: wgpu-hal's
/// `MAX_COLOR_ATTACHMENTS`, the bound wgpu builds its passes to.
const COLORS: usize = 8;

fn load_op<V>(load: u32, clear: V) -> wgpu::LoadOp<V> {
    match LOAD_OP.get(load as usize).copied() {
        Some("clear") => wgpu::LoadOp::Clear(clear),
        _ => wgpu::LoadOp::Load,
    }
}

fn store_op(store: u32) -> wgpu::StoreOp {
    match STORE_OP.get(store as usize).copied() {
        Some("discard") => wgpu::StoreOp::Discard,
        _ => wgpu::StoreOp::Store,
    }
}

fn slice(b: &wgpu::Buffer, offset: u64, size: Option<u64>) -> wgpu::BufferSlice<'_> {
    match size {
        Some(s) => b.slice(offset..offset + s),
        None => b.slice(offset..),
    }
}

/// A copy's buffer side with its buffer held, so the encoder can be borrowed.
struct OwnedBuffer {
    buffer: wgpu::Buffer,
    layout: wgpu::TexelCopyBufferLayout,
}

impl OwnedBuffer {
    fn info(&self) -> wgpu::TexelCopyBufferInfo<'_> {
        wgpu::TexelCopyBufferInfo {
            buffer: &self.buffer,
            layout: self.layout,
        }
    }
}

/// A copy's texture side with its texture held.
struct OwnedTexture {
    texture: wgpu::Texture,
    mip_level: u32,
    origin: wgpu::Origin3d,
    aspect: wgpu::TextureAspect,
}

impl OwnedTexture {
    fn info(&self) -> wgpu::TexelCopyTextureInfo<'_> {
        wgpu::TexelCopyTextureInfo {
            texture: &self.texture,
            mip_level: self.mip_level,
            origin: self.origin,
            aspect: self.aspect,
        }
    }
}

/// A limit's value as WebGPU names it, held to a word.
fn limit(l: &wgpu::Limits, name: &str) -> u32 {
    let v: u64 = match name {
        "maxTextureDimension1D" => l.max_texture_dimension_1d.into(),
        "maxTextureDimension2D" => l.max_texture_dimension_2d.into(),
        "maxTextureDimension3D" => l.max_texture_dimension_3d.into(),
        "maxTextureArrayLayers" => l.max_texture_array_layers.into(),
        "maxBindGroups" => l.max_bind_groups.into(),
        "maxBindGroupsPlusVertexBuffers" => l.max_bind_groups_plus_vertex_buffers.into(),
        "maxBindingsPerBindGroup" => l.max_bindings_per_bind_group.into(),
        "maxDynamicUniformBuffersPerPipelineLayout" => l.max_dynamic_uniform_buffers_per_pipeline_layout.into(),
        "maxDynamicStorageBuffersPerPipelineLayout" => l.max_dynamic_storage_buffers_per_pipeline_layout.into(),
        "maxSampledTexturesPerShaderStage" => l.max_sampled_textures_per_shader_stage.into(),
        "maxSamplersPerShaderStage" => l.max_samplers_per_shader_stage.into(),
        "maxStorageBuffersPerShaderStage" => l.max_storage_buffers_per_shader_stage.into(),
        "maxStorageTexturesPerShaderStage" => l.max_storage_textures_per_shader_stage.into(),
        "maxUniformBuffersPerShaderStage" => l.max_uniform_buffers_per_shader_stage.into(),
        "maxUniformBufferBindingSize" => l.max_uniform_buffer_binding_size,
        "maxStorageBufferBindingSize" => l.max_storage_buffer_binding_size,
        "minUniformBufferOffsetAlignment" => l.min_uniform_buffer_offset_alignment.into(),
        "minStorageBufferOffsetAlignment" => l.min_storage_buffer_offset_alignment.into(),
        "maxVertexBuffers" => l.max_vertex_buffers.into(),
        "maxBufferSize" => l.max_buffer_size,
        "maxVertexAttributes" => l.max_vertex_attributes.into(),
        "maxVertexBufferArrayStride" => l.max_vertex_buffer_array_stride.into(),
        "maxInterStageShaderVariables" => l.max_inter_stage_shader_variables.into(),
        "maxColorAttachments" => l.max_color_attachments.into(),
        "maxColorAttachmentBytesPerSample" => l.max_color_attachment_bytes_per_sample.into(),
        "maxComputeWorkgroupStorageSize" => l.max_compute_workgroup_storage_size.into(),
        "maxComputeInvocationsPerWorkgroup" => l.max_compute_invocations_per_workgroup.into(),
        "maxComputeWorkgroupSizeX" => l.max_compute_workgroup_size_x.into(),
        "maxComputeWorkgroupSizeY" => l.max_compute_workgroup_size_y.into(),
        "maxComputeWorkgroupSizeZ" => l.max_compute_workgroup_size_z.into(),
        "maxComputeWorkgroupsPerDimension" => l.max_compute_workgroups_per_dimension.into(),
        _ => 0,
    };
    v.min(u64::from(u32::MAX)) as u32
}

/// A feature as WebGPU names it; none for one wgpu does not offer.
fn feature(name: &str) -> Option<wgpu::Features> {
    Some(match name {
        "depth-clip-control" => wgpu::Features::DEPTH_CLIP_CONTROL,
        "depth32float-stencil8" => wgpu::Features::DEPTH32FLOAT_STENCIL8,
        "texture-compression-bc" => wgpu::Features::TEXTURE_COMPRESSION_BC,
        "texture-compression-bc-sliced-3d" => wgpu::Features::TEXTURE_COMPRESSION_BC_SLICED_3D,
        "texture-compression-etc2" => wgpu::Features::TEXTURE_COMPRESSION_ETC2,
        "texture-compression-astc" => wgpu::Features::TEXTURE_COMPRESSION_ASTC,
        "texture-compression-astc-sliced-3d" => wgpu::Features::TEXTURE_COMPRESSION_ASTC_SLICED_3D,
        "timestamp-query" => wgpu::Features::TIMESTAMP_QUERY,
        "indirect-first-instance" => wgpu::Features::INDIRECT_FIRST_INSTANCE,
        "shader-f16" => wgpu::Features::SHADER_F16,
        "rg11b10ufloat-renderable" => wgpu::Features::RG11B10UFLOAT_RENDERABLE,
        "bgra8unorm-storage" => wgpu::Features::BGRA8UNORM_STORAGE,
        "float32-filterable" => wgpu::Features::FLOAT32_FILTERABLE,
        "float32-blendable" => wgpu::Features::FLOAT32_BLENDABLE,
        "clip-distances" => wgpu::Features::CLIP_DISTANCES,
        "dual-source-blending" => wgpu::Features::DUAL_SOURCE_BLENDING,
        "subgroups" => wgpu::Features::SUBGROUP,
        _ => return None,
    })
}

#[cfg(test)]
#[path = "host.test.rs"]
mod test;
