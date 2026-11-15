// xpute-gpu-guest/encode.rs

//! The encoder writes a guest's GPU calls as records of a stream (stream.rs)
//! in the words gpu/link/spec/api.json5 gives each op. An object the
//! guest makes is named by a handle from one table (`Gpu`). A record that does
//! not fit an open transaction is not written, and the handle of the object it
//! would have made is given back.
//!
//! One function gives each record's words, and it runs twice: once to count
//! the words, then to write them into the room the stream reserved. So no
//! record is built on the heap first.

use core::sync::atomic::{AtomicU32, Ordering};

use xpute_link::mem;

use crate::handle::Table;
use crate::stream::{Stream, WORD};

use xpute_gpu_link::enums::{
    AddressMode, BlendFactor, BlendOperation, BufferBindingType, CanvasAlphaMode, CompareFunction,
    CullMode, FilterMode, FrontFace, IndexFormat, LoadOp, MipmapFilterMode, PrimitiveTopology,
    QueryType, SamplerBindingType, StencilOperation, StorageTextureAccess, StoreOp, TextureAspect,
    TextureDimension, TextureFormat, TextureSampleType, TextureViewDimension, VertexFormat,
    VertexStepMode,
};
use xpute_gpu_link::spec::{op, BindingKind, ResourceKind, NONE, SURFACE_PREFERRED};
use xpute_kit::abi::word::split64;

/// An `Id` is an object the guest made, named by a handle from its encoder's
/// table.
pub type Id = u32;

/// Loads a word that a record named for the host to answer, such as a
/// fence or the device block's `STATUS`. The load is an acquire, so whatever
/// the host wrote before it stored the word is seen (stream.rs). The word is
/// reached through a raw pointer (`AtomicU32::from_ptr`), so no reference is
/// made to a word the host writes.
///
/// # Safety
/// `word` is live and aligned, the host stores into it only atomically, and
/// nothing here writes through it.
pub unsafe fn load_acquire(word: *const u32) -> u32 {
    // SAFETY: the caller's.
    unsafe { AtomicU32::from_ptr(word.cast_mut()) }.load(Ordering::Acquire)
}

/// Whether the host has answered the request asked with `ticket` on
/// `fence` (spec `RESULT`). A late answer to an older request stores another
/// ticket, so it never counts.
///
/// # Safety
/// As `load_acquire`.
pub unsafe fn signaled(fence: *const u32, ticket: u32) -> bool {
    // SAFETY: the caller's.
    unsafe { load_acquire(fence) == ticket }
}

trait Sink {
    fn word(&mut self, v: u32);
    /// Bytes go as their count, then the bytes padded to whole words.
    fn bytes(&mut self, b: &[u8]);
}

struct Count(usize);

impl Sink for Count {
    fn word(&mut self, _: u32) {
        self.0 += 1;
    }
    fn bytes(&mut self, b: &[u8]) {
        self.0 += 1 + b.len().div_ceil(4);
    }
}

struct Fill<'w> {
    w: &'w mut [u32],
    at: usize,
}

impl Sink for Fill<'_> {
    fn word(&mut self, v: u32) {
        self.w[self.at] = v;
        self.at += 1;
    }
    fn bytes(&mut self, b: &[u8]) {
        self.word(b.len() as u32);
        let n = b.len().div_ceil(4);
        let words = &mut self.w[self.at..self.at + n];
        words.fill(0);
        for (k, byte) in b.iter().enumerate() {
            words[k / 4] |= u32::from(*byte) << (8 * (k % 4));
        }
        self.at += n;
    }
}

fn f(v: f32) -> u32 {
    v.to_bits()
}

fn opt(v: Option<u32>) -> u32 {
    v.unwrap_or(NONE)
}

fn flag(v: bool) -> u32 {
    u32::from(v)
}

/// Writes a record of `op` with the words `put` gives. It answers false where
/// an open transaction no longer fits the record; outside a transaction, a
/// record that does not fit is a bug (`Stream::reserve`).
fn emit(stream: &mut Stream, op: u32, put: &dyn Fn(&mut dyn Sink)) -> bool {
    let mut count = Count(0);
    put(&mut count);
    let Some(w) = stream.reserve(op, count.0) else {
        return false;
    };
    let mut fill = Fill { w, at: 0 };
    put(&mut fill);
    xpute_kit::ensure!(fill.at == count.0, ENOTRECOVERABLE, op);
    true
}

/// This is the room a record takes of a stream, its op and count words
/// included, counted as `emit` writes it.
fn rec_sz(put: &dyn Fn(&mut dyn Sink)) -> usize {
    let mut count = Count(0);
    put(&mut count);
    (2 + count.0) * WORD
}

pub struct BufferDescriptor {
    pub size: u32,
    /// These are `spec::buffer_usage` bits.
    pub usage: u32,
    pub mapped_at_creation: bool,
}

pub struct TextureDescriptor<'a> {
    /// This holds the width, the height, and the depth or layers.
    pub size: [u32; 3],
    pub mip_cnt: u32,
    pub sample_cnt: u32,
    pub dimension: TextureDimension,
    pub format: TextureFormat,
    /// These are `spec::texture_usage` bits.
    pub usage: u32,
    pub view_formats: &'a [TextureFormat],
}

#[derive(Default)]
pub struct ViewDescriptor {
    pub format: Option<TextureFormat>,
    pub dimension: Option<TextureViewDimension>,
    pub aspect: Option<TextureAspect>,
    pub base_mip: u32,
    pub mip_cnt: Option<u32>,
    pub base_layer: u32,
    pub layer_cnt: Option<u32>,
}

pub struct SamplerDescriptor {
    pub addr: [AddressMode; 3],
    pub mag: FilterMode,
    pub min: FilterMode,
    pub mipmap: MipmapFilterMode,
    pub lod: [f32; 2],
    pub compare: Option<CompareFunction>,
    pub max_anisotropy: u32,
}

pub enum BindingLayout {
    Buffer {
        ty: BufferBindingType,
        dynamic_offset: bool,
        min_binding_size: u32,
    },
    Sampler(SamplerBindingType),
    Texture {
        sample: TextureSampleType,
        dimension: TextureViewDimension,
        multisampled: bool,
    },
    StorageTexture {
        access: StorageTextureAccess,
        format: TextureFormat,
        dimension: TextureViewDimension,
    },
}

pub struct BindGroupLayoutEntry {
    pub binding: u32,
    /// These are `spec::shader_stage` bits.
    pub visibility: u32,
    pub layout: BindingLayout,
}

#[derive(Clone, Copy)]
pub enum Resource {
    Buffer {
        buffer: Id,
        offset: u32,
        size: Option<u32>,
    },
    Sampler(Id),
    View(Id),
}

#[derive(Clone, Copy)]
pub struct BindGroupEntry {
    pub binding: u32,
    pub resource: Resource,
}

pub struct VertexAttribute {
    pub format: VertexFormat,
    pub offset: u32,
    pub location: u32,
}

pub struct VertexBufferLayout<'a> {
    pub stride: u32,
    pub step: VertexStepMode,
    pub attributes: &'a [VertexAttribute],
}

pub struct VertexState<'a> {
    pub module: Id,
    pub entry: &'a str,
    pub buffers: &'a [VertexBufferLayout<'a>],
}

pub struct PrimitiveState {
    pub topology: PrimitiveTopology,
    pub strip_index_format: Option<IndexFormat>,
    pub front_face: FrontFace,
    pub cull: CullMode,
    pub unclipped_depth: bool,
}

pub struct StencilFaceState {
    pub compare: CompareFunction,
    pub fail: StencilOperation,
    pub depth_fail: StencilOperation,
    pub pass: StencilOperation,
}

/// This is WebGPU's default for a stencil face.
pub const STENCIL_UNUSED: StencilFaceState = StencilFaceState {
    compare: CompareFunction::Always,
    fail: StencilOperation::Keep,
    depth_fail: StencilOperation::Keep,
    pass: StencilOperation::Keep,
};

pub struct DepthStencilState {
    pub format: TextureFormat,
    pub depth_write: bool,
    pub depth_compare: CompareFunction,
    pub stencil_front: StencilFaceState,
    pub stencil_back: StencilFaceState,
    pub stencil_read_mask: u32,
    pub stencil_write_mask: u32,
    pub bias: i32,
    pub slope_scale: f32,
    pub clamp: f32,
}

pub struct MultisampleState {
    pub count: u32,
    pub mask: u32,
    pub alpha_to_coverage: bool,
}

/// This is WebGPU's default: one sample, no multisampling.
pub const MULTISAMPLE_NONE: MultisampleState = MultisampleState {
    count: 1,
    mask: u32::MAX,
    alpha_to_coverage: false,
};

pub struct BlendComponent {
    pub operation: BlendOperation,
    pub src: BlendFactor,
    pub dst: BlendFactor,
}

pub struct BlendState {
    pub color: BlendComponent,
    pub alpha: BlendComponent,
}

pub struct ColorTargetState {
    pub format: TextureFormat,
    pub blend: Option<BlendState>,
    /// These are `spec::color_write` bits.
    pub write_mask: u32,
}

pub struct FragmentState<'a> {
    pub module: Id,
    pub entry: &'a str,
    pub targets: &'a [ColorTargetState],
}

pub struct RenderPipelineDescriptor<'a> {
    /// None asks the device for a layout made from the shaders, WebGPU's
    /// "auto".
    pub layout: Option<Id>,
    pub vertex: VertexState<'a>,
    pub primitive: PrimitiveState,
    pub depth_stencil: Option<DepthStencilState>,
    pub multisample: MultisampleState,
    pub fragment: Option<FragmentState<'a>>,
}

pub struct ComputePipelineDescriptor<'a> {
    pub layout: Option<Id>,
    pub module: Id,
    pub entry: &'a str,
}

/// This is the place in a texture that a copy reads or writes.
pub struct TexelCopyTexture {
    pub texture: Id,
    pub mip: u32,
    pub origin: [u32; 3],
    pub aspect: TextureAspect,
}

/// These are a buffer's bytes, laid out as rows of texels for a copy.
pub struct TexelCopyBuffer {
    pub buffer: Id,
    pub offset: u32,
    pub bytes_per_row: u32,
    pub rows_per_image: u32,
}

pub struct ColorAttachment {
    pub view: Id,
    pub resolve: Option<Id>,
    pub load: LoadOp,
    pub store: StoreOp,
    pub clear: [f32; 4],
}

pub struct DepthStencilAttachment {
    pub view: Id,
    pub depth_load: LoadOp,
    pub depth_store: StoreOp,
    pub depth_clear: f32,
    pub depth_read_only: bool,
    pub stencil_load: LoadOp,
    pub stencil_store: StoreOp,
    pub stencil_clear: u32,
    pub stencil_read_only: bool,
}

pub struct TimestampWrites {
    pub query_set: Id,
    pub begin: Option<u32>,
    pub end: Option<u32>,
}

pub struct RenderPassDescriptor<'a> {
    pub colors: &'a [ColorAttachment],
    pub depth_stencil: Option<DepthStencilAttachment>,
    pub timestamps: Option<TimestampWrites>,
    pub occlusion: Option<Id>,
}

fn timestamps(s: &mut dyn Sink, t: &Option<TimestampWrites>) {
    match t {
        Some(t) => [t.query_set, opt(t.begin), opt(t.end)]
            .into_iter()
            .for_each(|w| s.word(w)),
        None => (0..3).for_each(|_| s.word(NONE)),
    }
}

fn texture_place(s: &mut dyn Sink, t: &TexelCopyTexture) {
    [
        t.texture,
        t.mip,
        t.origin[0],
        t.origin[1],
        t.origin[2],
        t.aspect as u32,
    ]
    .into_iter()
    .for_each(|w| s.word(w));
}

fn buffer_layout(s: &mut dyn Sink, b: &TexelCopyBuffer) {
    [b.buffer, b.offset, b.bytes_per_row, b.rows_per_image]
        .into_iter()
        .for_each(|w| s.word(w));
}

fn extent(s: &mut dyn Sink, e: [u32; 3]) {
    e.into_iter().for_each(|w| s.word(w));
}

fn stencil_face(s: &mut dyn Sink, f: &StencilFaceState) {
    [
        f.compare as u32,
        f.fail as u32,
        f.depth_fail as u32,
        f.pass as u32,
    ]
    .into_iter()
    .for_each(|w| s.word(w));
}

/// These are the words of a `RENDER_PIPELINE` record after its id, which
/// `RENDER_PIPELINE_ASYNC` begins with too.
fn render_pipeline_words(s: &mut dyn Sink, d: &RenderPipelineDescriptor) {
    s.word(opt(d.layout));
    let v = &d.vertex;
    s.word(v.module);
    s.bytes(v.entry.as_bytes());
    s.word(v.buffers.len() as u32);
    for b in v.buffers {
        [b.stride, b.step as u32, b.attributes.len() as u32]
            .into_iter()
            .for_each(|w| s.word(w));
        for a in b.attributes {
            [a.format as u32, a.offset, a.location]
                .into_iter()
                .for_each(|w| s.word(w));
        }
    }
    let p = &d.primitive;
    [
        p.topology as u32,
        opt(p.strip_index_format.map(|f| f as u32)),
        p.front_face as u32,
        p.cull as u32,
        flag(p.unclipped_depth),
    ]
    .into_iter()
    .for_each(|w| s.word(w));
    match &d.depth_stencil {
        Some(ds) => {
            [
                ds.format as u32,
                flag(ds.depth_write),
                ds.depth_compare as u32,
            ]
            .into_iter()
            .for_each(|w| s.word(w));
            stencil_face(s, &ds.stencil_front);
            stencil_face(s, &ds.stencil_back);
            [
                ds.stencil_read_mask,
                ds.stencil_write_mask,
                ds.bias as u32,
                f(ds.slope_scale),
                f(ds.clamp),
            ]
            .into_iter()
            .for_each(|w| s.word(w));
        }
        None => {
            // The depth-stencil words are present either way, so the
            // record's shape is fixed.
            s.word(NONE);
            (0..2 + 8 + 5).for_each(|_| s.word(0));
        }
    }
    let m = &d.multisample;
    [m.count, m.mask, flag(m.alpha_to_coverage)]
        .into_iter()
        .for_each(|w| s.word(w));
    match &d.fragment {
        Some(fr) => {
            s.word(fr.module);
            s.bytes(fr.entry.as_bytes());
            s.word(fr.targets.len() as u32);
            for t in fr.targets {
                s.word(t.format as u32);
                match &t.blend {
                    Some(b) => [
                        b.color.operation as u32,
                        b.color.src as u32,
                        b.color.dst as u32,
                        b.alpha.operation as u32,
                        b.alpha.src as u32,
                        b.alpha.dst as u32,
                    ]
                    .into_iter()
                    .for_each(|w| s.word(w)),
                    None => {
                        s.word(NONE);
                        (0..5).for_each(|_| s.word(0));
                    }
                }
                s.word(t.write_mask);
            }
        }
        None => {
            s.word(NONE);
            s.bytes(&[]);
            s.word(0);
        }
    }
}

/// These are the words of a `COMPUTE_PIPELINE` record after its id, which
/// `COMPUTE_PIPELINE_ASYNC` begins with too.
fn compute_pipeline_words(s: &mut dyn Sink, d: &ComputePipelineDescriptor) {
    [opt(d.layout), d.module]
        .into_iter()
        .for_each(|w| s.word(w));
    s.bytes(d.entry.as_bytes());
}

/// The ops numbered below `DESTROY` make objects that last until the guest
/// destroys them. The ops past it make what an entry lets go: an encoder, a
/// pass, a surface's current texture.
pub const OBJECT_OP_CNT: usize = op::DESTROY as usize;

/// These are the calls that make objects, over a table of `N` handles. Each
/// handle holds the op that made its object where the object lasts
/// (`OBJECT_OP_CNT`), and 0 where an entry lets it go. A `Gpu` whose bytes are all
/// zero holds no object, as `new` makes it.
pub struct Gpu<const N: usize> {
    handles: Table<u8, N>,
    /// This counts how many live objects each op has made, indexed by the op.
    objects: [u32; OBJECT_OP_CNT],
}

fn forget(objects: &mut [u32; OBJECT_OP_CNT], op: u8) {
    if op != 0 {
        objects[op as usize] -= 1;
    }
}

impl<const N: usize> Default for Gpu<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> Gpu<N> {
    pub const fn new() -> Self {
        Gpu {
            handles: Table::new(),
            objects: [0; OBJECT_OP_CNT],
        }
    }

    /// Makes an object by a record of `op` whose first word is the new
    /// handle. It answers none where the table is full or the record does not
    /// fit.
    fn make(&mut self, stream: &mut Stream, op: u32, put: &dyn Fn(&mut dyn Sink)) -> Option<Id> {
        self.make_as(stream, op, op, put)
    }

    /// This is `make`, but the object is counted under the op `kind`: an async
    /// pipeline counts as its sync op's.
    fn make_as(
        &mut self,
        stream: &mut Stream,
        op: u32,
        kind: u32,
        put: &dyn Fn(&mut dyn Sink),
    ) -> Option<Id> {
        let lasts = (kind as usize) < OBJECT_OP_CNT;
        let id = self.handles.insert(if lasts { kind as u8 } else { 0 })?;
        if !emit(stream, op, &|s| {
            s.word(id);
            put(s);
        }) {
            self.handles.remove(id);
            return None;
        }
        if lasts {
            self.objects[kind as usize] += 1;
        }
        Some(id)
    }

    /// Frees the handle `id`, and says whether it was live.
    fn release(&mut self, id: Id) -> bool {
        let made_by = self.handles.remove(id);
        if let Some(op) = made_by {
            forget(&mut self.objects, op);
        }
        made_by.is_some()
    }

    pub fn buffer(&mut self, stream: &mut Stream, d: &BufferDescriptor) -> Option<Id> {
        self.make(stream, op::BUFFER, &|s| {
            [d.size, d.usage, flag(d.mapped_at_creation)]
                .into_iter()
                .for_each(|w| s.word(w))
        })
    }

    pub fn texture(&mut self, stream: &mut Stream, d: &TextureDescriptor) -> Option<Id> {
        self.make(stream, op::TEXTURE, &|s| {
            [
                d.size[0],
                d.size[1],
                d.size[2],
                d.mip_cnt,
                d.sample_cnt,
                d.dimension as u32,
                d.format as u32,
                d.usage,
                d.view_formats.len() as u32,
            ]
            .into_iter()
            .for_each(|w| s.word(w));
            d.view_formats.iter().for_each(|f| s.word(*f as u32));
        })
    }

    pub fn view(&mut self, stream: &mut Stream, texture: Id, d: &ViewDescriptor) -> Option<Id> {
        self.make(stream, op::VIEW, &|s| {
            [
                texture,
                opt(d.format.map(|f| f as u32)),
                opt(d.dimension.map(|v| v as u32)),
                d.aspect.unwrap_or(TextureAspect::All) as u32,
                d.base_mip,
                opt(d.mip_cnt),
                d.base_layer,
                opt(d.layer_cnt),
            ]
            .into_iter()
            .for_each(|w| s.word(w))
        })
    }

    pub fn sampler(&mut self, stream: &mut Stream, d: &SamplerDescriptor) -> Option<Id> {
        self.make(stream, op::SAMPLER, &|s| {
            [
                d.addr[0] as u32,
                d.addr[1] as u32,
                d.addr[2] as u32,
                d.mag as u32,
                d.min as u32,
                d.mipmap as u32,
                f(d.lod[0]),
                f(d.lod[1]),
                opt(d.compare.map(|c| c as u32)),
                d.max_anisotropy,
            ]
            .into_iter()
            .for_each(|w| s.word(w))
        })
    }

    pub fn shader_module(&mut self, stream: &mut Stream, wgsl: &str) -> Option<Id> {
        self.make(stream, op::SHADER_MODULE, &|s| s.bytes(wgsl.as_bytes()))
    }

    /// This is the room `shader_module` takes of a stream, for `Stream::fits`,
    /// so that a caller making several modules can ask before each one.
    pub fn shader_module_sz(wgsl: &str) -> usize {
        rec_sz(&|s| {
            s.word(0);
            s.bytes(wgsl.as_bytes());
        })
    }

    pub fn bind_group_layout(
        &mut self,
        stream: &mut Stream,
        entries: &[BindGroupLayoutEntry],
    ) -> Option<Id> {
        self.make(stream, op::BIND_GROUP_LAYOUT, &|s| {
            s.word(entries.len() as u32);
            for e in entries {
                let (kind, a, b, c) = match e.layout {
                    BindingLayout::Buffer {
                        ty,
                        dynamic_offset,
                        min_binding_size,
                    } => (
                        BindingKind::Buffer,
                        ty as u32,
                        flag(dynamic_offset),
                        min_binding_size,
                    ),
                    BindingLayout::Sampler(ty) => (BindingKind::Sampler, ty as u32, 0, 0),
                    BindingLayout::Texture {
                        sample,
                        dimension,
                        multisampled,
                    } => (
                        BindingKind::Texture,
                        sample as u32,
                        dimension as u32,
                        flag(multisampled),
                    ),
                    BindingLayout::StorageTexture {
                        access,
                        format,
                        dimension,
                    } => (
                        BindingKind::StorageTexture,
                        access as u32,
                        format as u32,
                        dimension as u32,
                    ),
                };
                [e.binding, e.visibility, kind as u32, a, b, c]
                    .into_iter()
                    .for_each(|w| s.word(w));
            }
        })
    }

    pub fn pipeline_layout(&mut self, stream: &mut Stream, layouts: &[Id]) -> Option<Id> {
        self.make(stream, op::PIPELINE_LAYOUT, &|s| {
            s.word(layouts.len() as u32);
            layouts.iter().for_each(|l| s.word(*l));
        })
    }

    pub fn bind_group(
        &mut self,
        stream: &mut Stream,
        layout: Id,
        entries: &[BindGroupEntry],
    ) -> Option<Id> {
        self.make(stream, op::BIND_GROUP, &|s| {
            [layout, entries.len() as u32]
                .into_iter()
                .for_each(|w| s.word(w));
            for e in entries {
                let (kind, id, offset, size) = match e.resource {
                    Resource::Buffer {
                        buffer,
                        offset,
                        size,
                    } => (ResourceKind::Buffer, buffer, offset, opt(size)),
                    Resource::Sampler(id) => (ResourceKind::Sampler, id, 0, 0),
                    Resource::View(id) => (ResourceKind::TextureView, id, 0, 0),
                };
                [e.binding, kind as u32, id, offset, size]
                    .into_iter()
                    .for_each(|w| s.word(w));
            }
        })
    }

    pub fn render_pipeline(
        &mut self,
        stream: &mut Stream,
        d: &RenderPipelineDescriptor,
    ) -> Option<Id> {
        self.make(stream, op::RENDER_PIPELINE, &|s| {
            render_pipeline_words(s, d)
        })
    }

    /// This is `render_pipeline`, but the host makes the pipeline without
    /// holding its thread, and stores `ticket` at `fence` once it is made
    /// (spec `RESULT`). No record may name the pipeline before the guest has
    /// seen that ticket.
    pub fn render_pipeline_async(
        &mut self,
        stream: &mut Stream,
        d: &RenderPipelineDescriptor,
        fence: &u32,
        ticket: u32,
    ) -> Option<Id> {
        let [lo, hi] = split64(mem::addr(fence));
        self.make_as(
            stream,
            op::RENDER_PIPELINE_ASYNC,
            op::RENDER_PIPELINE,
            &|s| {
                render_pipeline_words(s, d);
                [lo, hi, ticket].into_iter().for_each(|w| s.word(w));
            },
        )
    }

    pub fn compute_pipeline(
        &mut self,
        stream: &mut Stream,
        d: &ComputePipelineDescriptor,
    ) -> Option<Id> {
        self.make(stream, op::COMPUTE_PIPELINE, &|s| {
            compute_pipeline_words(s, d)
        })
    }

    /// This is `compute_pipeline`, with the fence and ticket of
    /// `render_pipeline_async`.
    pub fn compute_pipeline_async(
        &mut self,
        stream: &mut Stream,
        d: &ComputePipelineDescriptor,
        fence: &u32,
        ticket: u32,
    ) -> Option<Id> {
        let [lo, hi] = split64(mem::addr(fence));
        self.make_as(
            stream,
            op::COMPUTE_PIPELINE_ASYNC,
            op::COMPUTE_PIPELINE,
            &|s| {
                compute_pipeline_words(s, d);
                [lo, hi, ticket].into_iter().for_each(|w| s.word(w));
            },
        )
    }

    pub fn query_set(&mut self, stream: &mut Stream, ty: QueryType, count: u32) -> Option<Id> {
        self.make(stream, op::QUERY_SET, &|s| {
            [ty as u32, count].into_iter().for_each(|w| s.word(w))
        })
    }

    pub fn encoder(&mut self, stream: &mut Stream) -> Option<Id> {
        self.make(stream, op::ENCODER, &|_| {})
    }

    pub fn render_pass(
        &mut self,
        stream: &mut Stream,
        encoder: Id,
        d: &RenderPassDescriptor,
    ) -> Option<Id> {
        self.make(stream, op::RENDER_PASS, &|s| {
            [encoder, d.colors.len() as u32]
                .into_iter()
                .for_each(|w| s.word(w));
            for c in d.colors {
                [
                    c.view,
                    opt(c.resolve),
                    c.load as u32,
                    c.store as u32,
                    f(c.clear[0]),
                    f(c.clear[1]),
                    f(c.clear[2]),
                    f(c.clear[3]),
                ]
                .into_iter()
                .for_each(|w| s.word(w));
            }
            match &d.depth_stencil {
                Some(ds) => [
                    ds.view,
                    ds.depth_load as u32,
                    ds.depth_store as u32,
                    f(ds.depth_clear),
                    flag(ds.depth_read_only),
                    ds.stencil_load as u32,
                    ds.stencil_store as u32,
                    ds.stencil_clear,
                    flag(ds.stencil_read_only),
                ]
                .into_iter()
                .for_each(|w| s.word(w)),
                None => {
                    s.word(NONE);
                    (0..8).for_each(|_| s.word(0));
                }
            }
            timestamps(s, &d.timestamps);
            s.word(opt(d.occlusion));
        })
    }

    pub fn compute_pass(
        &mut self,
        stream: &mut Stream,
        encoder: Id,
        t: &Option<TimestampWrites>,
    ) -> Option<Id> {
        self.make(stream, op::COMPUTE_PASS, &|s| {
            s.word(encoder);
            timestamps(s, t);
        })
    }

    /// This makes a view of `surface`'s texture for this entry. Every `CURRENT`
    /// of one entry names the same texture.
    pub fn current(&mut self, stream: &mut Stream, surface: u32) -> Option<Id> {
        self.make(stream, op::CURRENT, &|s| s.word(surface))
    }

    /// Opens a transaction. The records from here to `commit` are written
    /// whole or not at all, and so are the objects they make.
    pub fn begin(&mut self, stream: &mut Stream) {
        stream.begin();
        self.handles.begin();
    }

    /// Closes the transaction. It answers false where it did not fit: the
    /// stream is then as it was at `begin`, and every handle taken since is
    /// free again, since the host never saw it.
    pub fn commit(&mut self, stream: &mut Stream) -> bool {
        let fit = stream.commit();
        if !fit {
            let objects = &mut self.objects;
            self.handles.rollback(|_, op| forget(objects, op));
        }
        fit
    }

    /// Ends a pass and frees its handle, whether or not the record was
    /// written: a pass dropped with its transaction left nothing on the host
    /// to end.
    pub fn end(&mut self, stream: &mut Stream, pass: Id) -> bool {
        let sent = emit(stream, op::END, &|s| s.word(pass));
        self.release(pass);
        sent
    }

    /// Submits the encoders in order, and frees their handles as `end`
    /// frees a pass's.
    pub fn submit(&mut self, stream: &mut Stream, encoders: &[Id]) -> bool {
        let sent = emit(stream, op::SUBMIT, &|s| {
            s.word(encoders.len() as u32);
            encoders.iter().for_each(|e| s.word(*e));
        });
        encoders.iter().for_each(|e| {
            self.release(*e);
        });
        sent
    }

    pub fn destroy(&mut self, stream: &mut Stream, id: Id) {
        if self.release(id) {
            emit(stream, op::DESTROY, &|s| s.word(id));
        }
    }

    pub fn live(&self, id: Id) -> bool {
        self.handles.live(id)
    }

    pub fn objects(&self) -> &[u32; OBJECT_OP_CNT] {
        &self.objects
    }
}

/// These are the calls that make no object. Each writes its record straight to
/// the stream, and returns false where an open transaction no longer fits it.
pub mod call {
    use super::*;

    fn words(stream: &mut Stream, op: u32, w: &[u32]) -> bool {
        emit(stream, op, &|s| w.iter().for_each(|v| s.word(*v)))
    }

    fn buffer_write(s: &mut dyn Sink, buffer: Id, offset: u32, bytes: &[u8]) {
        [buffer, offset].into_iter().for_each(|w| s.word(w));
        s.bytes(bytes);
    }

    pub fn write_buffer(stream: &mut Stream, buffer: Id, offset: u32, bytes: &[u8]) -> bool {
        emit(stream, op::WRITE_BUFFER, &|s| {
            buffer_write(s, buffer, offset, bytes)
        })
    }

    /// This is the room `write_buffer` takes of a stream, for `Stream::fits`,
    /// so that a caller can ask once whether several writes fit.
    pub fn write_buffer_sz(bytes: &[u8]) -> usize {
        rec_sz(&|s| buffer_write(s, 0, 0, bytes))
    }

    /// This is `write_buffer`, but the record names the bytes where they lie in
    /// the guest's memory, and the host reads them there as it runs the
    /// stream. The guest leaves them unchanged until the stream is handed back
    /// (stream.rs).
    pub fn write_buffer_at(stream: &mut Stream, buffer: Id, offset: u32, bytes: &[u8]) -> bool {
        let (at, len) = mem::span(bytes);
        let [lo, hi] = split64(at);
        words(stream, op::WRITE_BUFFER_AT, &[buffer, offset, lo, hi, len])
    }

    fn texture_write(s: &mut dyn Sink, place: &TexelCopyTexture, layout: [u32; 2], size: [u32; 3]) {
        texture_place(s, place);
        layout.into_iter().for_each(|w| s.word(w));
        extent(s, size);
    }

    /// `layout` holds the bytes per row and the rows per image.
    pub fn write_texture(
        stream: &mut Stream,
        place: &TexelCopyTexture,
        layout: [u32; 2],
        size: [u32; 3],
        bytes: &[u8],
    ) -> bool {
        emit(stream, op::WRITE_TEXTURE, &|s| {
            texture_write(s, place, layout, size);
            s.bytes(bytes);
        })
    }

    /// This is `write_buffer_sz` for a texture. The room depends on the
    /// bytes alone, not on the place, layout or size.
    pub fn write_texture_sz(bytes: &[u8]) -> usize {
        let place = TexelCopyTexture {
            texture: 0,
            mip: 0,
            origin: [0; 3],
            aspect: TextureAspect::All,
        };
        rec_sz(&|s| {
            texture_write(s, &place, [0; 2], [0; 3]);
            s.bytes(bytes);
        })
    }

    pub fn write_texture_at(
        stream: &mut Stream,
        place: &TexelCopyTexture,
        layout: [u32; 2],
        size: [u32; 3],
        bytes: &[u8],
    ) -> bool {
        let (at, len) = mem::span(bytes);
        let [lo, hi] = split64(at);
        emit(stream, op::WRITE_TEXTURE_AT, &|s| {
            texture_write(s, place, layout, size);
            [lo, hi, len].into_iter().for_each(|w| s.word(w));
        })
    }

    /// Decodes an encoded image (PNG, JPEG or WebP) from the guest's memory
    /// into `to`. `fence` is the fence and the result word after it (spec
    /// `RESULT`): the host stores `ticket` there once the image is in the
    /// texture or refused. The host copies the image's bytes as it runs the
    /// record, so they are the guest's again once the stream is handed back.
    pub fn copy_external_image(
        stream: &mut Stream,
        to: &TexelCopyTexture,
        flip_y: bool,
        premultiplied: bool,
        image: &[u8],
        fence: &[u32; 2],
        ticket: u32,
    ) -> bool {
        let (image_at, image_len) = mem::span(image);
        let ([image_lo, image_hi], [word_lo, word_hi]) =
            (split64(image_at), split64(mem::addr(fence)));
        emit(stream, op::COPY_EXTERNAL_IMAGE, &|s| {
            texture_place(s, to);
            [
                flag(flip_y),
                flag(premultiplied),
                image_lo,
                image_hi,
                image_len,
                word_lo,
                word_hi,
                ticket,
            ]
            .into_iter()
            .for_each(|w| s.word(w));
        })
    }

    pub fn copy_buffer_to_buffer(
        stream: &mut Stream,
        encoder: Id,
        from: (Id, u32),
        to: (Id, u32),
        size: u32,
    ) -> bool {
        words(
            stream,
            op::COPY_BUFFER_TO_BUFFER,
            &[encoder, from.0, from.1, to.0, to.1, size],
        )
    }

    pub fn copy_buffer_to_texture(
        stream: &mut Stream,
        encoder: Id,
        from: &TexelCopyBuffer,
        to: &TexelCopyTexture,
        size: [u32; 3],
    ) -> bool {
        emit(stream, op::COPY_BUFFER_TO_TEXTURE, &|s| {
            s.word(encoder);
            buffer_layout(s, from);
            texture_place(s, to);
            extent(s, size);
        })
    }

    pub fn copy_texture_to_buffer(
        stream: &mut Stream,
        encoder: Id,
        from: &TexelCopyTexture,
        to: &TexelCopyBuffer,
        size: [u32; 3],
    ) -> bool {
        emit(stream, op::COPY_TEXTURE_TO_BUFFER, &|s| {
            s.word(encoder);
            texture_place(s, from);
            buffer_layout(s, to);
            extent(s, size);
        })
    }

    pub fn copy_texture_to_texture(
        stream: &mut Stream,
        encoder: Id,
        from: &TexelCopyTexture,
        to: &TexelCopyTexture,
        size: [u32; 3],
    ) -> bool {
        emit(stream, op::COPY_TEXTURE_TO_TEXTURE, &|s| {
            s.word(encoder);
            texture_place(s, from);
            texture_place(s, to);
            extent(s, size);
        })
    }

    pub fn clear_buffer(
        stream: &mut Stream,
        encoder: Id,
        buffer: Id,
        offset: u32,
        size: Option<u32>,
    ) -> bool {
        words(
            stream,
            op::CLEAR_BUFFER,
            &[encoder, buffer, offset, opt(size)],
        )
    }

    pub fn resolve_query_set(
        stream: &mut Stream,
        encoder: Id,
        query_set: Id,
        first: u32,
        count: u32,
        buffer: Id,
        offset: u32,
    ) -> bool {
        words(
            stream,
            op::RESOLVE_QUERY_SET,
            &[encoder, query_set, first, count, buffer, offset],
        )
    }

    /// `pipeline` is a render or a compute pipeline, as `pass` is.
    pub fn set_pipeline(stream: &mut Stream, pass: Id, pipeline: Id) -> bool {
        words(stream, op::SET_PIPELINE, &[pass, pipeline])
    }

    pub fn set_bind_group(
        stream: &mut Stream,
        pass: Id,
        index: u32,
        group: Id,
        dynamic_offsets: &[u32],
    ) -> bool {
        emit(stream, op::SET_BIND_GROUP, &|s| {
            [pass, index, group, dynamic_offsets.len() as u32]
                .into_iter()
                .for_each(|w| s.word(w));
            dynamic_offsets.iter().for_each(|o| s.word(*o));
        })
    }

    pub fn set_vertex_buffer(
        stream: &mut Stream,
        pass: Id,
        slot: u32,
        buffer: Id,
        offset: u32,
        size: Option<u32>,
    ) -> bool {
        words(
            stream,
            op::SET_VERTEX_BUFFER,
            &[pass, slot, buffer, offset, opt(size)],
        )
    }

    pub fn set_index_buffer(
        stream: &mut Stream,
        pass: Id,
        buffer: Id,
        format: IndexFormat,
        offset: u32,
        size: Option<u32>,
    ) -> bool {
        words(
            stream,
            op::SET_INDEX_BUFFER,
            &[pass, buffer, format as u32, offset, opt(size)],
        )
    }

    /// `rect` holds x, y, width and height, and `depth` holds the min and the
    /// max.
    pub fn set_viewport(stream: &mut Stream, pass: Id, rect: [f32; 4], depth: [f32; 2]) -> bool {
        words(
            stream,
            op::SET_VIEWPORT,
            &[
                pass,
                f(rect[0]),
                f(rect[1]),
                f(rect[2]),
                f(rect[3]),
                f(depth[0]),
                f(depth[1]),
            ],
        )
    }

    pub fn set_scissor(stream: &mut Stream, pass: Id, rect: [u32; 4]) -> bool {
        words(
            stream,
            op::SET_SCISSOR,
            &[pass, rect[0], rect[1], rect[2], rect[3]],
        )
    }

    pub fn set_blend_constant(stream: &mut Stream, pass: Id, rgba: [f32; 4]) -> bool {
        words(
            stream,
            op::SET_BLEND_CONSTANT,
            &[pass, f(rgba[0]), f(rgba[1]), f(rgba[2]), f(rgba[3])],
        )
    }

    pub fn set_stencil_reference(stream: &mut Stream, pass: Id, reference: u32) -> bool {
        words(stream, op::SET_STENCIL_REFERENCE, &[pass, reference])
    }

    pub fn draw(
        stream: &mut Stream,
        pass: Id,
        vertex_count: u32,
        instance_count: u32,
        first_vertex: u32,
        first_instance: u32,
    ) -> bool {
        words(
            stream,
            op::DRAW,
            &[
                pass,
                vertex_count,
                instance_count,
                first_vertex,
                first_instance,
            ],
        )
    }

    pub fn draw_indexed(
        stream: &mut Stream,
        pass: Id,
        index_count: u32,
        instance_count: u32,
        first_index: u32,
        base_vertex: i32,
        first_instance: u32,
    ) -> bool {
        words(
            stream,
            op::DRAW_INDEXED,
            &[
                pass,
                index_count,
                instance_count,
                first_index,
                base_vertex as u32,
                first_instance,
            ],
        )
    }

    pub fn draw_indirect(stream: &mut Stream, pass: Id, buffer: Id, offset: u32) -> bool {
        words(stream, op::DRAW_INDIRECT, &[pass, buffer, offset])
    }

    pub fn draw_indexed_indirect(stream: &mut Stream, pass: Id, buffer: Id, offset: u32) -> bool {
        words(stream, op::DRAW_INDEXED_INDIRECT, &[pass, buffer, offset])
    }

    pub fn dispatch(stream: &mut Stream, pass: Id, workgroups: [u32; 3]) -> bool {
        words(
            stream,
            op::DISPATCH,
            &[pass, workgroups[0], workgroups[1], workgroups[2]],
        )
    }

    pub fn dispatch_indirect(stream: &mut Stream, pass: Id, buffer: Id, offset: u32) -> bool {
        words(stream, op::DISPATCH_INDIRECT, &[pass, buffer, offset])
    }

    /// Names `id`, any object, encoder or pass made before, as `label` in the
    /// device's messages. A native host ignores it, since wgpu names an
    /// object only as it makes it.
    pub fn label(stream: &mut Stream, id: Id, label: &str) -> bool {
        emit(stream, op::LABEL, &|s| {
            s.word(id);
            s.bytes(label.as_bytes());
        })
    }

    /// Reads `buffer` into the guest's memory at `into`. The buffer has
    /// MAP_READ and COPY_DST usage, and an encoder submitted before copied
    /// into it. `fence` is the fence and the result word after it (spec
    /// `RESULT`): the host stores `ticket` there once the bytes are in
    /// `into` and the buffer is unmapped, or once the mapping is refused.
    pub fn map_async(
        stream: &mut Stream,
        buffer: Id,
        offset: u32,
        into: &mut [u8],
        fence: &[u32; 2],
        ticket: u32,
    ) -> bool {
        let (at, len) = mem::span(into);
        let ([lo, hi], [word_lo, word_hi]) = (split64(at), split64(mem::addr(fence)));
        words(
            stream,
            op::MAP_ASYNC,
            &[buffer, offset, len, lo, hi, word_lo, word_hi, ticket],
        )
    }

    /// Asks the host to store `ticket` at `fence` once the work submitted
    /// before is done (spec `RESULT`).
    pub fn queue_done(stream: &mut Stream, fence: &u32, ticket: u32) -> bool {
        let [lo, hi] = split64(mem::addr(fence));
        words(stream, op::QUEUE_DONE, &[lo, hi, ticket])
    }

    /// `format` None asks for the format the host's surface prefers.
    pub fn configure(
        stream: &mut Stream,
        surface: u32,
        format: Option<TextureFormat>,
        usage: u32,
        alpha: CanvasAlphaMode,
    ) -> bool {
        words(
            stream,
            op::CONFIGURE,
            &[
                surface,
                format.map_or(SURFACE_PREFERRED, |f| f as u32),
                usage,
                alpha as u32,
            ],
        )
    }

    pub fn resize(stream: &mut Stream, surface: u32, size: [u32; 2]) -> bool {
        words(stream, op::RESIZE, &[surface, size[0], size[1]])
    }
}
