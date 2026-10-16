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

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use serde::de::value::{Error as NameError, StrDeserializer};
use serde::de::{DeserializeOwned, IntoDeserializer};
use xpute_guest::abi::handle::handle_slot;

use crate::enums::{
    ADDRESS_MODE, BLEND_FACTOR, BLEND_OPERATION, BUFFER_BINDING_TYPE, CANVAS_ALPHA_MODE, COMPARE_FUNCTION, CULL_MODE, FEATURE_NAME, FILTER_MODE, FRONT_FACE, INDEX_FORMAT, LIMIT_NAME, LOAD_OP,
    MIPMAP_FILTER_MODE, PRIMITIVE_TOPOLOGY, QUERY_TYPE, SAMPLER_BINDING_TYPE, STENCIL_OPERATION, STORAGE_TEXTURE_ACCESS, STORE_OP, TEXTURE_ASPECT, TEXTURE_DIMENSION, TEXTURE_FORMAT,
    TEXTURE_SAMPLE_TYPE, TEXTURE_VIEW_DIMENSION, VERTEX_FORMAT, VERTEX_STEP_MODE,
};
use crate::spec::{op, DONE, NONE, STATUS, SURFACE_PREFERRED};

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

    /// The names wgpu reads none of: what a guest asks by them it cannot be given.
    fn unread(&self) -> Vec<&'static str> {
        fn none<'a, T>(names: &'static [&'static str], of: &'a [Option<T>]) -> impl Iterator<Item = &'static str> + 'a {
            names.iter().zip(of).filter(|(_, v)| v.is_none()).map(|(n, _)| *n)
        }
        let mut out = Vec::new();
        out.extend(none(TEXTURE_FORMAT, &self.texture_format));
        out.extend(none(VERTEX_FORMAT, &self.vertex_format));
        out.extend(none(PRIMITIVE_TOPOLOGY, &self.topology));
        out.extend(none(INDEX_FORMAT, &self.index_format));
        out.extend(none(FRONT_FACE, &self.front_face));
        out.extend(none(COMPARE_FUNCTION, &self.compare));
        out.extend(none(STENCIL_OPERATION, &self.stencil));
        out.extend(none(BLEND_OPERATION, &self.blend_operation));
        out.extend(none(BLEND_FACTOR, &self.blend_factor));
        out.extend(none(VERTEX_STEP_MODE, &self.step));
        out.extend(none(ADDRESS_MODE, &self.address));
        out.extend(none(FILTER_MODE, &self.filter));
        out.extend(none(MIPMAP_FILTER_MODE, &self.mipmap));
        out.extend(none(TEXTURE_DIMENSION, &self.dimension));
        out.extend(none(TEXTURE_VIEW_DIMENSION, &self.view_dimension));
        out.extend(none(TEXTURE_ASPECT, &self.aspect));
        out.extend(none(SAMPLER_BINDING_TYPE, &self.sampler_binding));
        out.extend(none(STORAGE_TEXTURE_ACCESS, &self.storage_access));
        out
    }
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

impl Words<'_> {
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
    /// None where the guest left it unset.
    fn opt(&mut self) -> Option<u32> {
        Some(self.u()).filter(|&v| v != NONE)
    }
    /// A count, then the bytes padded to words.
    fn bytes(&mut self) -> Vec<u8> {
        let n = self.u() as usize;
        let words = &self.w[self.at..self.at + n.div_ceil(4)];
        self.at += n.div_ceil(4);
        words.iter().flat_map(|w| w.to_le_bytes()).take(n).collect()
    }
    fn text(&mut self) -> String {
        String::from_utf8_lossy(&self.bytes()).into_owned()
    }
}

/// A read or a done the device answers at a later poll.
struct Pending {
    state: Arc<AtomicU8>,
    word: u32,
    ticket: u32,
    /// A read's staging buffer and where its bytes go.
    read: Option<(wgpu::Buffer, u32)>,
}

const WAITING: u8 = 0;
const ANSWERED: u8 = 1;
const REFUSED: u8 = 2;

/// A surface the guest names by number, as the program bound it.
struct Surface {
    surface: wgpu::Surface<'static>,
    config: Option<wgpu::SurfaceConfiguration>,
    size: [u32; 2],
    /// This turn's texture, presented at the turn's end (`present`).
    frame: Option<wgpu::SurfaceTexture>,
}

pub struct GpuHost {
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    names: Names,
    objects: Vec<Option<Object>>,
    /// At the number the program binds each by, as objects at their slots.
    surfaces: Vec<Option<Surface>>,
    /// Where the guest asked the device's block written, for its status.
    block: Option<(u32, u32)>,
    pending: Vec<Pending>,
    /// A record's words, copied out of the guest's memory so the record may
    /// write into it.
    scratch: Vec<u32>,
}

/// The guest's memory, as the host reads and writes it at the addresses the
/// guest names.
fn word_at(memory: &[u8], at: u32) -> u32 {
    let a = at as usize;
    u32::from_le_bytes([memory[a], memory[a + 1], memory[a + 2], memory[a + 3]])
}

fn set_word(memory: &mut [u8], at: u32, v: u32) {
    let a = at as usize;
    memory[a..a + 4].copy_from_slice(&v.to_le_bytes());
}

impl GpuHost {
    pub fn new(adapter: wgpu::Adapter, device: wgpu::Device, queue: wgpu::Queue) -> GpuHost {
        GpuHost {
            adapter,
            device,
            queue,
            names: Names::new(),
            objects: Vec::new(),
            surfaces: Vec::new(),
            block: None,
            pending: Vec::new(),
            scratch: Vec::new(),
        }
    }

    /// The spec's names wgpu gives no value of, which a guest naming them
    /// cannot be given.
    pub fn unread_names(&self) -> Vec<&'static str> {
        self.names.unread()
    }

    /// A surface the guest names by `surface` in CONFIGURE, RESIZE and
    /// CURRENT, `size` its pixels as the window has it.
    pub fn bind_surface(&mut self, surface: u32, s: wgpu::Surface<'static>, size: [u32; 2]) {
        let at = surface as usize;
        if self.surfaces.len() <= at {
            self.surfaces.resize_with(at + 1, || None);
        }
        self.surfaces[at] = Some(Surface {
            surface: s,
            config: None,
            size,
            frame: None,
        });
    }

    pub fn unbind_surface(&mut self, surface: u32) {
        if let Some(s) = self.surfaces.get_mut(surface as usize) {
            *s = None;
        }
    }

    /// The device went: every object with it, and the guest told in its block.
    pub fn lost(&mut self, memory: &mut [u8]) {
        self.objects.clear();
        self.pending.clear();
        if let Some((address, words)) = self.block {
            set_word(memory, address + 4 * (words - 1), STATUS::LOST);
        }
    }

    /// What the device has answered since: each read's bytes copied to its
    /// address and each done's word set, where the ticket still stands.
    /// Run at the turn's edge, before the turn reads its words.
    pub fn poll(&mut self, memory: &mut [u8]) {
        let _ = self.device.poll(wgpu::PollType::Poll);
        self.pending.retain(|p| match p.state.load(Ordering::Acquire) {
            WAITING => true,
            ANSWERED => {
                if word_at(memory, p.word) == p.ticket {
                    if let Some((staging, address)) = &p.read {
                        if let Ok(range) = staging.slice(..).get_mapped_range() {
                            let a = *address as usize;
                            memory[a..a + range.len()].copy_from_slice(&range);
                        }
                    }
                    set_word(memory, p.word, p.ticket | DONE);
                }
                false
            }
            _ => false,
        });
    }

    /// Each surface's texture this turn drew, shown.
    pub fn present(&mut self) {
        for s in self.surfaces.iter_mut().flatten() {
            if let Some(frame) = s.frame.take() {
                self.queue.present(frame);
            }
        }
    }

    /// Runs the stream at `at`, `head` words of head, as the turn left it:
    /// word 0 of the head the count of record words past it, each record its
    /// op, its count of words, then they.
    pub fn run(&mut self, memory: &mut [u8], at: u32, head: u32) {
        let end = word_at(memory, at);
        let first = at + 4 * head;
        let mut k = 0;
        while k + 2 <= end {
            let code = word_at(memory, first + 4 * k);
            let n = word_at(memory, first + 4 * (k + 1));
            let mut scratch = core::mem::take(&mut self.scratch);
            scratch.clear();
            let from = (first + 4 * (k + 2)) as usize;
            scratch.extend(memory[from..from + 4 * n as usize].as_chunks::<4>().0.iter().map(|w| u32::from_le_bytes(*w)));
            self.record(memory, code, &mut Words { w: &scratch, at: 0 });
            self.scratch = scratch;
            k += 2 + n;
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

    fn record(&mut self, memory: &mut [u8], code: u32, r: &mut Words) {
        match code {
            op::BUFFER => {
                let id = r.u();
                let (size, usage, mapped) = (u64::from(r.u()), r.u(), r.flag());
                let b = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: None,
                    size,
                    usage: wgpu::BufferUsages::from_bits_truncate(usage),
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
                let usage = wgpu::TextureUsages::from_bits_truncate(r.u());
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
                let v = texture.create_view(&wgpu::TextureViewDescriptor {
                    label: None,
                    format,
                    dimension,
                    usage: None,
                    aspect,
                    base_mip_level,
                    mip_level_count,
                    base_array_layer,
                    array_layer_count,
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
                            visibility: wgpu::ShaderStages::from_bits_truncate(visibility),
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
                self.queue.write_buffer(self.buffer(id), offset, &bytes);
            }
            op::WRITE_BUFFER_AT => {
                let (id, offset, address, size) = (r.u(), u64::from(r.u()), r.u() as usize, r.u() as usize);
                self.queue.write_buffer(self.buffer(id), offset, &memory[address..address + size]);
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
                    self.queue.write_texture(place.info(), &bytes, layout, size);
                } else {
                    let (address, n) = (r.u() as usize, r.u() as usize);
                    self.queue.write_texture(place.info(), &memory[address..address + n], layout, size);
                }
            }
            op::COPY_EXTERNAL_IMAGE => {
                let to = self.texture_place(r);
                let (flip_y, premultiplied) = (r.flag(), r.flag());
                let (address, n, word, ticket) = (r.u() as usize, r.u() as usize, r.u(), r.u());
                // An image this host cannot decode is left out, its word not
                // written, as the web leaves out one its browser cannot.
                if let Some(image) = crate::image::decode(&memory[address..address + n], flip_y, premultiplied) {
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
                    self.queue.write_texture(to.info(), &image.rgba, layout, size);
                    // In the texture as the queue's next work sees it, so
                    // answered at the next poll.
                    self.pending.push(Pending {
                        state: Arc::new(AtomicU8::new(ANSWERED)),
                        word,
                        ticket,
                        read: None,
                    });
                }
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
                let (pass, index, group) = (r.u(), r.u(), r.u());
                let offsets: Vec<u32> = (0..r.u()).map(|_| r.u()).collect();
                let group = match self.get(group) {
                    Object::BindGroup(g) => g.clone(),
                    _ => panic!("[gpu] {group} is no bind group"),
                };
                match self.get_mut(pass) {
                    Object::RenderPass(p) => p.set_bind_group(index, &group, &offsets),
                    Object::ComputePass(p) => p.set_bind_group(index, &group, &offsets),
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
                let ids: Vec<u32> = (0..r.u()).map(|_| r.u()).collect();
                let finished: Vec<wgpu::CommandBuffer> = ids
                    .iter()
                    .filter_map(|&id| match self.take(id) {
                        Some(Object::Encoder(e)) => Some(e.finish()),
                        _ => None,
                    })
                    .collect();
                self.queue.submit(finished);
            }
            op::READ_BUFFER => {
                let (b, offset, size, address, word, ticket) = (r.u(), u64::from(r.u()), r.u(), r.u(), r.u(), r.u());
                let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: None,
                    size: u64::from(size),
                    usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                let mut e = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
                e.copy_buffer_to_buffer(self.buffer(b), offset, &staging, 0, u64::from(size));
                self.queue.submit([e.finish()]);
                let state = Arc::new(AtomicU8::new(WAITING));
                let answered = Arc::clone(&state);
                staging.slice(..).map_async(wgpu::MapMode::Read, move |result| {
                    answered.store(if result.is_ok() { ANSWERED } else { REFUSED }, Ordering::Release);
                });
                self.pending.push(Pending {
                    state,
                    word,
                    ticket,
                    read: Some((staging, address)),
                });
            }
            op::QUEUE_DONE => {
                let (word, ticket) = (r.u(), r.u());
                let state = Arc::new(AtomicU8::new(WAITING));
                let answered = Arc::clone(&state);
                self.queue.on_submitted_work_done(move || answered.store(ANSWERED, Ordering::Release));
                self.pending.push(Pending { state, word, ticket, read: None });
            }
            op::CONFIGURE => {
                let surface = r.u();
                let format = r.u();
                let usage = wgpu::TextureUsages::from_bits_truncate(r.u());
                let alpha = r.u();
                let Some(s) = self.surfaces.get_mut(surface as usize).and_then(Option::as_mut) else { return };
                // No format is the adapter unable to show on this surface at all.
                let caps = s.surface.get_capabilities(&self.adapter);
                assert!(!caps.formats.is_empty(), "[gpu] surface {surface} cannot be presented to by this adapter");
                let format = if format == SURFACE_PREFERRED {
                    caps.formats[0]
                } else {
                    of(&self.names.texture_format, format, "format")
                };
                let alpha_mode = match CANVAS_ALPHA_MODE.get(alpha as usize).copied() {
                    Some("premultiplied") => wgpu::CompositeAlphaMode::PreMultiplied,
                    _ => wgpu::CompositeAlphaMode::Opaque,
                };
                let alpha_mode = if caps.alpha_modes.contains(&alpha_mode) { alpha_mode } else { wgpu::CompositeAlphaMode::Auto };
                let config = wgpu::SurfaceConfiguration {
                    usage,
                    format,
                    color_space: wgpu::SurfaceColorSpace::Auto,
                    width: s.size[0].max(1),
                    height: s.size[1].max(1),
                    present_mode: wgpu::PresentMode::Fifo,
                    desired_maximum_frame_latency: 2,
                    alpha_mode,
                    view_formats: Vec::new(),
                };
                s.surface.configure(&self.device, &config);
                s.config = Some(config);
            }
            op::RESIZE => {
                let (surface, width, height) = (r.u(), r.u(), r.u());
                let Some(s) = self.surfaces.get_mut(surface as usize).and_then(Option::as_mut) else { return };
                s.size = [width, height];
                if let Some(c) = s.config.as_mut().filter(|c| c.width != width.max(1) || c.height != height.max(1)) {
                    (c.width, c.height) = (width.max(1), height.max(1));
                    s.frame = None;
                    s.surface.configure(&self.device, c);
                }
            }
            op::CURRENT => {
                let (id, surface) = (r.u(), r.u());
                let Some(s) = self.surfaces.get_mut(surface as usize).and_then(Option::as_mut) else { return };
                if s.frame.is_none() {
                    s.frame = match s.surface.get_current_texture() {
                        wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => Some(f),
                        _ => None,
                    };
                }
                let Some(frame) = &s.frame else { return };
                let format = frame.texture.format();
                let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
                self.put(id, Object::View(view, format));
            }
            op::DEVICE => {
                let (address, words) = (r.u(), r.u());
                self.block = Some((address, words));
                let (limits, features) = (self.device.limits(), self.device.features());
                for (k, name) in LIMIT_NAME.iter().enumerate() {
                    set_word(memory, address + 4 * k as u32, limit(&limits, name));
                }
                let mut bits = vec![0u32; FEATURE_NAME.len().div_ceil(32)];
                for (k, name) in FEATURE_NAME.iter().enumerate() {
                    if feature(name).is_some_and(|f| features.contains(f)) {
                        bits[k >> 5] |= 1 << (k & 31);
                    }
                }
                for (k, w) in bits.into_iter().enumerate() {
                    set_word(memory, address + 4 * (LIMIT_NAME.len() + k) as u32, w);
                }
                set_word(memory, address + 4 * (words - 1), STATUS::READY);
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
                    write_mask: wgpu::ColorWrites::from_bits_truncate(r.u()),
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
        let colors: Vec<Color> = (0..r.u())
            .map(|_| {
                let view = self.view(r.u()).0.clone();
                let resolve = r.opt().map(|v| self.view(v).0.clone());
                let (load, store) = (r.u(), r.u());
                let c = [r.f32(), r.f32(), r.f32(), r.f32()].map(f64::from);
                let clear = wgpu::Color { r: c[0], g: c[1], b: c[2], a: c[3] };
                Color {
                    view,
                    resolve,
                    ops: wgpu::Operations {
                        load: load_op(load, clear),
                        store: store_op(store),
                    },
                }
            })
            .collect();
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
        let color_attachments: Vec<Option<wgpu::RenderPassColorAttachment>> = colors
            .iter()
            .map(|c| {
                Some(wgpu::RenderPassColorAttachment {
                    view: &c.view,
                    depth_slice: None,
                    resolve_target: c.resolve.as_ref(),
                    ops: c.ops,
                })
            })
            .collect();
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
                color_attachments: &color_attachments,
                depth_stencil_attachment,
                timestamp_writes,
                occlusion_query_set: occlusion.as_ref(),
                multiview_mask: None,
            })
            .forget_lifetime();
        self.put(id, Object::RenderPass(Box::new(p)));
    }
}

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
