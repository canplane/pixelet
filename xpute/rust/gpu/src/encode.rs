// xpute-gpu/encode.rs

//! A guest's GPU calls written as a stream's records (`ipc/stream`), in the
//! words spec/gpu/api.json gives each op. Objects are handles from one table;
//! a record that does not fit an open transaction is not written, and an
//! object it would have made is handed back.
//!
//! Every record is written twice by the same function: once to count its
//! words, then into the room the stream reserved for them, so nothing is
//! built on the heap first.

use xpute_guest::abi::handle::{handle_slot, HandleTable};
use xpute_guest::ipc::stream::{Stream, WORD};
use xpute_guest::mem::off_of_slice;

use crate::enums::{
    AddressMode, BlendFactor, BlendOperation, BufferBindingType, CanvasAlphaMode, CompareFunction, CullMode, FilterMode, FrontFace, IndexFormat, LoadOp, MipmapFilterMode, PrimitiveTopology,
    QueryType, SamplerBindingType, StencilOperation, StorageTextureAccess, StoreOp, TextureAspect, TextureDimension, TextureFormat, TextureSampleType, TextureViewDimension, VertexFormat,
    VertexStepMode,
};
use crate::spec::{op, BindingKind, ResourceKind, NONE, SURFACE_PREFERRED};

/// An object the guest made: a handle of the table its encoder holds.
pub type Id = u32;

/// Where a record's words go: counted, or written.
trait Sink {
    fn word(&mut self, v: u32);
    /// A count, then the bytes padded to words.
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

/// Writes a record of `op` whose words `put` gives; false where an open
/// transaction no longer fits it.
fn emit(stream: &mut Stream, op: u32, put: &dyn Fn(&mut dyn Sink)) -> bool {
    let mut count = Count(0);
    put(&mut count);
    let Some(w) = stream.reserve(op, count.0) else { return false };
    let mut fill = Fill { w, at: 0 };
    put(&mut fill);
    xpute_kit::ensure!(fill.at == count.0, ENOTRECOVERABLE, op);
    true
}

/// The bytes a record of `op` whose words `put` gives takes of a stream, as
/// its room is counted: measured as `emit` measures it, its op and count with.
fn measured(put: &dyn Fn(&mut dyn Sink)) -> usize {
    let mut count = Count(0);
    put(&mut count);
    (2 + count.0) * WORD
}

pub struct BufferDescriptor {
    pub size: u32,
    /// `spec::BUFFER_USAGE` bits.
    pub usage: u32,
    pub mapped_at_creation: bool,
}

pub struct TextureDescriptor<'a> {
    /// Width, height, and depth or layers.
    pub size: [u32; 3],
    pub mip_levels: u32,
    pub samples: u32,
    pub dimension: TextureDimension,
    pub format: TextureFormat,
    /// `spec::TEXTURE_USAGE` bits.
    pub usage: u32,
    pub view_formats: &'a [TextureFormat],
}

#[derive(Default)]
pub struct ViewDescriptor {
    pub format: Option<TextureFormat>,
    pub dimension: Option<TextureViewDimension>,
    pub aspect: Option<TextureAspect>,
    pub base_mip: u32,
    pub mips: Option<u32>,
    pub base_layer: u32,
    pub layers: Option<u32>,
}

pub struct SamplerDescriptor {
    pub address: [AddressMode; 3],
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
    /// `spec::SHADER_STAGE` bits.
    pub visibility: u32,
    pub layout: BindingLayout,
}

#[derive(Clone, Copy)]
pub enum Resource {
    Buffer { buffer: Id, offset: u32, size: Option<u32> },
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

/// What WebGPU defaults a face to: always, keep, keep, keep.
pub const STENCIL_UNUSED: StencilFaceState = StencilFaceState {
    compare: CompareFunction::always,
    fail: StencilOperation::keep,
    depth_fail: StencilOperation::keep,
    pass: StencilOperation::keep,
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

/// One sample, every sample's bit set.
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
    /// `spec::COLOR_WRITE` bits.
    pub write_mask: u32,
}

pub struct FragmentState<'a> {
    pub module: Id,
    pub entry: &'a str,
    pub targets: &'a [ColorTargetState],
}

pub struct RenderPipelineDescriptor<'a> {
    /// None for a layout the device makes from the shaders.
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

/// A texture's place a copy reads or writes.
pub struct TexelCopyTexture {
    pub texture: Id,
    pub mip: u32,
    pub origin: [u32; 3],
    pub aspect: TextureAspect,
}

/// A buffer's bytes laid out as texels.
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
        Some(t) => [t.query_set, opt(t.begin), opt(t.end)].into_iter().for_each(|w| s.word(w)),
        None => (0..3).for_each(|_| s.word(NONE)),
    }
}

fn texture_place(s: &mut dyn Sink, t: &TexelCopyTexture) {
    [t.texture, t.mip, t.origin[0], t.origin[1], t.origin[2], t.aspect as u32].into_iter().for_each(|w| s.word(w));
}

fn buffer_layout(s: &mut dyn Sink, b: &TexelCopyBuffer) {
    [b.buffer, b.offset, b.bytes_per_row, b.rows_per_image].into_iter().for_each(|w| s.word(w));
}

fn extent(s: &mut dyn Sink, e: [u32; 3]) {
    e.into_iter().for_each(|w| s.word(w));
}

fn stencil_face(s: &mut dyn Sink, f: &StencilFaceState) {
    [f.compare as u32, f.fail as u32, f.depth_fail as u32, f.pass as u32].into_iter().for_each(|w| s.word(w));
}

/// The ops below DESTROY make what lasts until it is destroyed, the objects a
/// program holds; those past it make what a turn lets go (an encoder, a
/// pass, a surface's current texture).
pub const OBJECT_OPS: usize = op::DESTROY as usize;

/// The calls, over a table of `N` handles. All-zero bytes are one that has
/// made nothing, as its table's are an empty table.
pub struct Gpu<const N: usize> {
    handles: HandleTable<N>,
    /// The op that made each slot's live object, where it lasts; 0 otherwise.
    made_by: [u8; N],
    /// The live objects by the op that made them, slot `op`.
    objects: [u32; OBJECT_OPS],
}

/// Slot `id`'s object let go from the count of what made it.
fn forget(made_by: &mut [u8], objects: &mut [u32; OBJECT_OPS], id: Id) {
    let op = core::mem::take(&mut made_by[handle_slot(id) as usize]);
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
            handles: HandleTable::new(),
            made_by: [0; N],
            objects: [0; OBJECT_OPS],
        }
    }

    /// An object's handle, written by `op` as the record's first word; none
    /// where the table is full or the record did not fit.
    fn make(&mut self, stream: &mut Stream, op: u32, put: &dyn Fn(&mut dyn Sink)) -> Option<Id> {
        let id = self.handles.acquire()?;
        if !emit(stream, op, &|s| {
            s.word(id);
            put(s);
        }) {
            self.handles.release(id);
            return None;
        }
        if (op as usize) < OBJECT_OPS {
            self.made_by[handle_slot(id) as usize] = op as u8;
            self.objects[op as usize] += 1;
        }
        Some(id)
    }

    /// `id` let go, where it was live.
    fn release(&mut self, id: Id) -> bool {
        let live = self.handles.release(id);
        if live {
            forget(&mut self.made_by, &mut self.objects, id);
        }
        live
    }

    pub fn buffer(&mut self, stream: &mut Stream, d: &BufferDescriptor) -> Option<Id> {
        self.make(stream, op::BUFFER, &|s| [d.size, d.usage, flag(d.mapped_at_creation)].into_iter().for_each(|w| s.word(w)))
    }

    pub fn texture(&mut self, stream: &mut Stream, d: &TextureDescriptor) -> Option<Id> {
        self.make(stream, op::TEXTURE, &|s| {
            [
                d.size[0],
                d.size[1],
                d.size[2],
                d.mip_levels,
                d.samples,
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
                d.aspect.unwrap_or(TextureAspect::all) as u32,
                d.base_mip,
                opt(d.mips),
                d.base_layer,
                opt(d.layers),
            ]
            .into_iter()
            .for_each(|w| s.word(w))
        })
    }

    pub fn sampler(&mut self, stream: &mut Stream, d: &SamplerDescriptor) -> Option<Id> {
        self.make(stream, op::SAMPLER, &|s| {
            [
                d.address[0] as u32,
                d.address[1] as u32,
                d.address[2] as u32,
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

    pub fn bind_group_layout(&mut self, stream: &mut Stream, entries: &[BindGroupLayoutEntry]) -> Option<Id> {
        self.make(stream, op::BIND_GROUP_LAYOUT, &|s| {
            s.word(entries.len() as u32);
            for e in entries {
                let (kind, a, b, c) = match e.layout {
                    BindingLayout::Buffer { ty, dynamic_offset, min_binding_size } => (BindingKind::buffer, ty as u32, flag(dynamic_offset), min_binding_size),
                    BindingLayout::Sampler(ty) => (BindingKind::sampler, ty as u32, 0, 0),
                    BindingLayout::Texture { sample, dimension, multisampled } => (BindingKind::texture, sample as u32, dimension as u32, flag(multisampled)),
                    BindingLayout::StorageTexture { access, format, dimension } => (BindingKind::storage_texture, access as u32, format as u32, dimension as u32),
                };
                [e.binding, e.visibility, kind as u32, a, b, c].into_iter().for_each(|w| s.word(w));
            }
        })
    }

    pub fn pipeline_layout(&mut self, stream: &mut Stream, layouts: &[Id]) -> Option<Id> {
        self.make(stream, op::PIPELINE_LAYOUT, &|s| {
            s.word(layouts.len() as u32);
            layouts.iter().for_each(|l| s.word(*l));
        })
    }

    pub fn bind_group(&mut self, stream: &mut Stream, layout: Id, entries: &[BindGroupEntry]) -> Option<Id> {
        self.make(stream, op::BIND_GROUP, &|s| {
            [layout, entries.len() as u32].into_iter().for_each(|w| s.word(w));
            for e in entries {
                let (kind, id, offset, size) = match e.resource {
                    Resource::Buffer { buffer, offset, size } => (ResourceKind::buffer, buffer, offset, opt(size)),
                    Resource::Sampler(id) => (ResourceKind::sampler, id, 0, 0),
                    Resource::View(id) => (ResourceKind::texture_view, id, 0, 0),
                };
                [e.binding, kind as u32, id, offset, size].into_iter().for_each(|w| s.word(w));
            }
        })
    }

    pub fn render_pipeline(&mut self, stream: &mut Stream, d: &RenderPipelineDescriptor) -> Option<Id> {
        self.make(stream, op::RENDER_PIPELINE, &|s| {
            s.word(opt(d.layout));
            let v = &d.vertex;
            s.word(v.module);
            s.bytes(v.entry.as_bytes());
            s.word(v.buffers.len() as u32);
            for b in v.buffers {
                [b.stride, b.step as u32, b.attributes.len() as u32].into_iter().for_each(|w| s.word(w));
                for a in b.attributes {
                    [a.format as u32, a.offset, a.location].into_iter().for_each(|w| s.word(w));
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
                    [ds.format as u32, flag(ds.depth_write), ds.depth_compare as u32].into_iter().for_each(|w| s.word(w));
                    stencil_face(s, &ds.stencil_front);
                    stencil_face(s, &ds.stencil_back);
                    [ds.stencil_read_mask, ds.stencil_write_mask, ds.bias as u32, f(ds.slope_scale), f(ds.clamp)]
                        .into_iter()
                        .for_each(|w| s.word(w));
                }
                None => {
                    // Present either way, so the record's shape is fixed.
                    s.word(NONE);
                    (0..2 + 8 + 5).for_each(|_| s.word(0));
                }
            }
            let m = &d.multisample;
            [m.count, m.mask, flag(m.alpha_to_coverage)].into_iter().for_each(|w| s.word(w));
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
        })
    }

    pub fn compute_pipeline(&mut self, stream: &mut Stream, d: &ComputePipelineDescriptor) -> Option<Id> {
        self.make(stream, op::COMPUTE_PIPELINE, &|s| {
            [opt(d.layout), d.module].into_iter().for_each(|w| s.word(w));
            s.bytes(d.entry.as_bytes());
        })
    }

    pub fn query_set(&mut self, stream: &mut Stream, ty: QueryType, count: u32) -> Option<Id> {
        self.make(stream, op::QUERY_SET, &|s| [ty as u32, count].into_iter().for_each(|w| s.word(w)))
    }

    pub fn encoder(&mut self, stream: &mut Stream) -> Option<Id> {
        self.make(stream, op::ENCODER, &|_| {})
    }

    pub fn render_pass(&mut self, stream: &mut Stream, encoder: Id, d: &RenderPassDescriptor) -> Option<Id> {
        self.make(stream, op::RENDER_PASS, &|s| {
            [encoder, d.colors.len() as u32].into_iter().for_each(|w| s.word(w));
            for c in d.colors {
                [c.view, opt(c.resolve), c.load as u32, c.store as u32, f(c.clear[0]), f(c.clear[1]), f(c.clear[2]), f(c.clear[3])]
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

    pub fn compute_pass(&mut self, stream: &mut Stream, encoder: Id, t: &Option<TimestampWrites>) -> Option<Id> {
        self.make(stream, op::COMPUTE_PASS, &|s| {
            s.word(encoder);
            timestamps(s, t);
        })
    }

    /// This turn's texture of `surface`, as a view.
    pub fn current(&mut self, stream: &mut Stream, surface: u32) -> Option<Id> {
        self.make(stream, op::CURRENT, &|s| s.word(surface))
    }

    /// The records from here to `commit` are one, and so are the objects they
    /// make: a transaction that does not fit drops both.
    pub fn begin(&mut self, stream: &mut Stream) {
        stream.begin();
        self.handles.begin();
    }

    /// False where the transaction did not fit: the stream is as at `begin`,
    /// and every handle taken since is free again, as the host never saw it.
    pub fn commit(&mut self, stream: &mut Stream) -> bool {
        let fit = stream.commit();
        if !fit {
            let (made_by, objects) = (&mut self.made_by, &mut self.objects);
            self.handles.rollback(|id| forget(made_by, objects, id));
        }
        fit
    }

    /// Ends a pass: the host lets it go with the record, and its handle is
    /// free again, whether or not the record was written, since one dropped
    /// with its transaction left nothing on the host.
    pub fn end(&mut self, stream: &mut Stream, pass: Id) -> bool {
        let sent = emit(stream, op::END, &|s| s.word(pass));
        self.release(pass);
        sent
    }

    /// Each encoder finished and submitted, in order, and let go as `end`
    /// lets a pass go.
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

    /// Lets the object go: the host destroys it, and its handle is free.
    pub fn destroy(&mut self, stream: &mut Stream, id: Id) {
        if self.release(id) {
            emit(stream, op::DESTROY, &|s| s.word(id));
        }
    }

    /// Whether `id` is an object this encoder made and has not let go.
    pub fn live(&self, id: Id) -> bool {
        self.handles.live(id)
    }

    /// The objects this encoder made and has not let go, by the op that made
    /// them (slot `op`, below DESTROY).
    pub fn objects(&self) -> &[u32; OBJECT_OPS] {
        &self.objects
    }
}

/// The calls that make no object: an op's words straight to the stream,
/// false where an open transaction no longer fits them.
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
        emit(stream, op::WRITE_BUFFER, &|s| buffer_write(s, buffer, offset, bytes))
    }

    /// The bytes `write_buffer` of `bytes` takes of a stream, which
    /// `Stream::fits` asks of its room: a caller writing several asks once.
    pub fn write_buffer_bytes(bytes: &[u8]) -> usize {
        measured(&|s| buffer_write(s, 0, 0, bytes))
    }

    /// The bytes read where they lie in the guest's memory when the host
    /// runs the stream, so they must stay until it has.
    pub fn write_buffer_at(stream: &mut Stream, buffer: Id, offset: u32, bytes: &[u8]) -> bool {
        words(stream, op::WRITE_BUFFER_AT, &[buffer, offset, off_of_slice(bytes) as u32, bytes.len() as u32])
    }

    fn texture_write(s: &mut dyn Sink, place: &TexelCopyTexture, layout: [u32; 2], size: [u32; 3]) {
        texture_place(s, place);
        layout.into_iter().for_each(|w| s.word(w));
        extent(s, size);
    }

    /// `layout`: bytes per row, rows per image.
    pub fn write_texture(stream: &mut Stream, place: &TexelCopyTexture, layout: [u32; 2], size: [u32; 3], bytes: &[u8]) -> bool {
        emit(stream, op::WRITE_TEXTURE, &|s| {
            texture_write(s, place, layout, size);
            s.bytes(bytes);
        })
    }

    /// The bytes `write_texture` of `bytes` takes of a stream, as
    /// `write_buffer_bytes` says a buffer's: its words are the same whatever
    /// the place, the layout and the size.
    pub fn write_texture_bytes(bytes: &[u8]) -> usize {
        let place = TexelCopyTexture {
            texture: 0,
            mip: 0,
            origin: [0; 3],
            aspect: TextureAspect::all,
        };
        measured(&|s| {
            texture_write(s, &place, [0; 2], [0; 3]);
            s.bytes(bytes);
        })
    }

    pub fn write_texture_at(stream: &mut Stream, place: &TexelCopyTexture, layout: [u32; 2], size: [u32; 3], bytes: &[u8]) -> bool {
        emit(stream, op::WRITE_TEXTURE_AT, &|s| {
            texture_write(s, place, layout, size);
            [off_of_slice(bytes) as u32, bytes.len() as u32].into_iter().for_each(|w| s.word(w));
        })
    }

    /// An encoded image (PNG, JPEG, WebP) where it lies in the guest's memory,
    /// decoded into `to`; `word` where `ticket | DONE` lands once it is there.
    pub fn copy_external_image(stream: &mut Stream, to: &TexelCopyTexture, flip_y: bool, premultiplied: bool, image: &[u8], word: &mut u32, ticket: u32) -> bool {
        *word = ticket;
        let (image_at, word_at) = (off_of_slice(image) as u32, off_of_slice(core::slice::from_ref(word)) as u32);
        emit(stream, op::COPY_EXTERNAL_IMAGE, &|s| {
            texture_place(s, to);
            [flag(flip_y), flag(premultiplied), image_at, image.len() as u32, word_at, ticket].into_iter().for_each(|w| s.word(w));
        })
    }

    pub fn copy_buffer_to_buffer(stream: &mut Stream, encoder: Id, from: (Id, u32), to: (Id, u32), size: u32) -> bool {
        words(stream, op::COPY_BUFFER_TO_BUFFER, &[encoder, from.0, from.1, to.0, to.1, size])
    }

    pub fn copy_buffer_to_texture(stream: &mut Stream, encoder: Id, from: &TexelCopyBuffer, to: &TexelCopyTexture, size: [u32; 3]) -> bool {
        emit(stream, op::COPY_BUFFER_TO_TEXTURE, &|s| {
            s.word(encoder);
            buffer_layout(s, from);
            texture_place(s, to);
            extent(s, size);
        })
    }

    pub fn copy_texture_to_buffer(stream: &mut Stream, encoder: Id, from: &TexelCopyTexture, to: &TexelCopyBuffer, size: [u32; 3]) -> bool {
        emit(stream, op::COPY_TEXTURE_TO_BUFFER, &|s| {
            s.word(encoder);
            texture_place(s, from);
            buffer_layout(s, to);
            extent(s, size);
        })
    }

    pub fn copy_texture_to_texture(stream: &mut Stream, encoder: Id, from: &TexelCopyTexture, to: &TexelCopyTexture, size: [u32; 3]) -> bool {
        emit(stream, op::COPY_TEXTURE_TO_TEXTURE, &|s| {
            s.word(encoder);
            texture_place(s, from);
            texture_place(s, to);
            extent(s, size);
        })
    }

    pub fn clear_buffer(stream: &mut Stream, encoder: Id, buffer: Id, offset: u32, size: Option<u32>) -> bool {
        words(stream, op::CLEAR_BUFFER, &[encoder, buffer, offset, opt(size)])
    }

    pub fn resolve_query_set(stream: &mut Stream, encoder: Id, query_set: Id, first: u32, count: u32, buffer: Id, offset: u32) -> bool {
        words(stream, op::RESOLVE_QUERY_SET, &[encoder, query_set, first, count, buffer, offset])
    }

    /// A render or a compute pipeline, as the pass is.
    pub fn set_pipeline(stream: &mut Stream, pass: Id, pipeline: Id) -> bool {
        words(stream, op::SET_PIPELINE, &[pass, pipeline])
    }

    pub fn set_bind_group(stream: &mut Stream, pass: Id, index: u32, group: Id, dynamic_offsets: &[u32]) -> bool {
        emit(stream, op::SET_BIND_GROUP, &|s| {
            [pass, index, group, dynamic_offsets.len() as u32].into_iter().for_each(|w| s.word(w));
            dynamic_offsets.iter().for_each(|o| s.word(*o));
        })
    }

    pub fn set_vertex_buffer(stream: &mut Stream, pass: Id, slot: u32, buffer: Id, offset: u32, size: Option<u32>) -> bool {
        words(stream, op::SET_VERTEX_BUFFER, &[pass, slot, buffer, offset, opt(size)])
    }

    pub fn set_index_buffer(stream: &mut Stream, pass: Id, buffer: Id, format: IndexFormat, offset: u32, size: Option<u32>) -> bool {
        words(stream, op::SET_INDEX_BUFFER, &[pass, buffer, format as u32, offset, opt(size)])
    }

    /// `rect`: x, y, width, height; `depth`: min and max.
    pub fn set_viewport(stream: &mut Stream, pass: Id, rect: [f32; 4], depth: [f32; 2]) -> bool {
        words(stream, op::SET_VIEWPORT, &[pass, f(rect[0]), f(rect[1]), f(rect[2]), f(rect[3]), f(depth[0]), f(depth[1])])
    }

    pub fn set_scissor(stream: &mut Stream, pass: Id, rect: [u32; 4]) -> bool {
        words(stream, op::SET_SCISSOR, &[pass, rect[0], rect[1], rect[2], rect[3]])
    }

    pub fn set_blend_constant(stream: &mut Stream, pass: Id, rgba: [f32; 4]) -> bool {
        words(stream, op::SET_BLEND_CONSTANT, &[pass, f(rgba[0]), f(rgba[1]), f(rgba[2]), f(rgba[3])])
    }

    pub fn set_stencil_reference(stream: &mut Stream, pass: Id, reference: u32) -> bool {
        words(stream, op::SET_STENCIL_REFERENCE, &[pass, reference])
    }

    pub fn draw(stream: &mut Stream, pass: Id, vertices: u32, instances: u32, first_vertex: u32, first_instance: u32) -> bool {
        words(stream, op::DRAW, &[pass, vertices, instances, first_vertex, first_instance])
    }

    pub fn draw_indexed(stream: &mut Stream, pass: Id, indices: u32, instances: u32, first_index: u32, base_vertex: i32, first_instance: u32) -> bool {
        words(stream, op::DRAW_INDEXED, &[pass, indices, instances, first_index, base_vertex as u32, first_instance])
    }

    pub fn draw_indirect(stream: &mut Stream, pass: Id, buffer: Id, offset: u32) -> bool {
        words(stream, op::DRAW_INDIRECT, &[pass, buffer, offset])
    }

    pub fn draw_indexed_indirect(stream: &mut Stream, pass: Id, buffer: Id, offset: u32) -> bool {
        words(stream, op::DRAW_INDEXED_INDIRECT, &[pass, buffer, offset])
    }

    pub fn dispatch(stream: &mut Stream, pass: Id, workgroups: [u32; 3]) -> bool {
        words(stream, op::DISPATCH, &[pass, workgroups[0], workgroups[1], workgroups[2]])
    }

    pub fn dispatch_indirect(stream: &mut Stream, pass: Id, buffer: Id, offset: u32) -> bool {
        words(stream, op::DISPATCH_INDIRECT, &[pass, buffer, offset])
    }

    /// `buffer`, made MAP_READ | COPY_DST and copied into on an encoder
    /// submitted before, mapped for reading: `into` the bytes' place in the
    /// guest's memory, `word` the mapping's own, where `ticket | DONE` lands
    /// once they are there and the buffer is unmapped again.
    pub fn map_async(stream: &mut Stream, buffer: Id, offset: u32, into: &mut [u8], word: &mut u32, ticket: u32) -> bool {
        *word = ticket;
        words(
            stream,
            op::MAP_ASYNC,
            &[buffer, offset, into.len() as u32, off_of_slice(into) as u32, off_of_slice(core::slice::from_ref(word)) as u32, ticket],
        )
    }

    /// `ticket | DONE` at `word` once what was submitted before is done.
    pub fn queue_done(stream: &mut Stream, word: &mut u32, ticket: u32) -> bool {
        *word = ticket;
        words(stream, op::QUEUE_DONE, &[off_of_slice(core::slice::from_ref(word)) as u32, ticket])
    }

    /// `format` None for the format the host's surface prefers.
    pub fn configure(stream: &mut Stream, surface: u32, format: Option<TextureFormat>, usage: u32, alpha: CanvasAlphaMode) -> bool {
        words(stream, op::CONFIGURE, &[surface, format.map_or(SURFACE_PREFERRED, |f| f as u32), usage, alpha as u32])
    }

    pub fn resize(stream: &mut Stream, surface: u32, size: [u32; 2]) -> bool {
        words(stream, op::RESIZE, &[surface, size[0], size[1]])
    }

    /// Where the host writes the device's limits, features and status.
    pub fn device(stream: &mut Stream, into: &mut [u32]) -> bool {
        words(stream, op::DEVICE, &[off_of_slice(into) as u32, into.len() as u32])
    }
}

#[cfg(test)]
#[path = "encode.test.rs"]
mod test;
