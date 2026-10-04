// pxl-render/gpu/api/device.rs

//! The renderer's calls made directly through wgpu, meaning what each record
//! means to the page, so the renderer cannot tell the two apart. A call naming
//! an object the device does not hold does nothing, as a WebGPU error does on the page.

use std::num::NonZeroU64;
use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

use xpute_runtime::abi::handle::handle_slot;
use xpute_runtime::global::Global;

use super::NONE;
use crate::frame::shader::record::VERTEX_ENTRY;
use crate::gpu::api_spec::{address, compare, dimension, filter, format, load, record, resource};

/// RGBA, top row first, unconverted and not premultiplied, as the page's bitmap is.
struct Image {
    width: u32,
    height: u32,
    pixels: Box<[u8]>,
}

/// Keyed by the whole handle, generation included, so a reissued slot names
/// only its new image. Locked because images decode off the kernel's thread.
static IMAGES: Mutex<Option<std::collections::HashMap<u32, Image>>> = Mutex::new(None);

/// False for a file that does not decode. Runs on any thread.
pub fn hold_image(handle: u32, file: &[u8]) -> bool {
    let image = if file.starts_with(b"RIFF") { decode_webp(file) } else { decode_png(file) };
    let Some(image) = image else { return false };
    IMAGES.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(Default::default).insert(handle, image);
    true
}

fn decode_png(file: &[u8]) -> Option<Image> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(file));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let frame = reader.next_frame(&mut buf).ok()?;
    let at = &buf[..frame.buffer_size()];
    let pixels: Box<[u8]> = match frame.color_type {
        png::ColorType::Rgba => at.into(),
        png::ColorType::Rgb => at.as_chunks::<3>().0.iter().flat_map(|&[r, g, b]| [r, g, b, 255]).collect(),
        png::ColorType::GrayscaleAlpha => at.as_chunks::<2>().0.iter().flat_map(|&[l, a]| [l, l, l, a]).collect(),
        png::ColorType::Grayscale => at.iter().flat_map(|&l| [l, l, l, 255]).collect(),
        png::ColorType::Indexed => return None,
    };
    Some(Image {
        width: frame.width,
        height: frame.height,
        pixels,
    })
}

fn decode_webp(file: &[u8]) -> Option<Image> {
    let mut decoder = image_webp::WebPDecoder::new(std::io::Cursor::new(file)).ok()?;
    let (width, height) = decoder.dimensions();
    let mut buf = vec![0; decoder.output_buffer_size()?];
    decoder.read_image(&mut buf).ok()?;
    let pixels: Box<[u8]> = if decoder.has_alpha() {
        buf.into()
    } else {
        buf.as_chunks::<3>().0.iter().flat_map(|&[r, g, b]| [r, g, b, 255]).collect()
    };
    Some(Image { width, height, pixels })
}

#[derive(Clone, Copy, Debug)]
pub struct DeviceInfo {
    pub align: u32,
    pub timestamps: bool,
}

enum Object {
    Buffer(wgpu::Buffer),
    Texture(wgpu::Texture),
    View(wgpu::TextureView),
    Sampler(wgpu::Sampler),
    Shader(wgpu::ShaderModule),
    Layout(wgpu::BindGroupLayout),
    Pipeline(wgpu::RenderPipeline),
    Group(wgpu::BindGroup),
    Encoder(wgpu::CommandEncoder),
    Pass(Box<wgpu::RenderPass<'static>>),
    QuerySet(wgpu::QuerySet, u32),
}

/// Resolved into one buffer and copied into one of three, so a pending read
/// does not hold up the next frame's.
struct Stamps {
    count: u32,
    resolve: wgpu::Buffer,
    reads: [StampRead; 3],
}

struct StampRead {
    buffer: wgpu::Buffer,
    /// FREE, COPIED (the copy is recorded, not yet mapped), MAPPING, MAPPED.
    state: Arc<AtomicU8>,
    written: u32,
}

const FREE: u8 = 0;
const COPIED: u8 = 1;
const MAPPING: u8 = 2;
const MAPPED: u8 = 3;

/// Binding configures: the renderer configures surfaces before the app's view
/// has a layer, so a bound surface is always one a frame can be taken from.
struct Surface {
    /// A slot outlives its handle; unbinding an old handle must not take the one bound since.
    handle: u32,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
}

struct Device {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    objects: Vec<Option<Object>>,
    surfaces: Vec<Option<Surface>>,
    /// Kept by slot so a layer bound later is configured at the last size given.
    sizes: Vec<Option<(u32, [u32; 2])>>,
    frames: Vec<wgpu::SurfaceTexture>,
    stamps: Option<Stamps>,
    gpu_ms: Vec<f64>,
    /// `Queue::write_buffer` makes a Metal buffer and mapping per write, a dozen a
    /// frame; the belt maps chunks once and reuses them.
    belt: wgpu::util::StagingBelt,
    /// Submitted ahead of the next encoder so a write lands before what reads it.
    uploads: Option<wgpu::CommandEncoder>,
    in_flight: Arc<AtomicU32>,
}

static DEVICE: Global<Option<Device>> = Global::new();

fn device() -> Option<&'static mut Device> {
    DEVICE.get(|| None).as_mut()
}

/// BGRA on Apple, RGBA on Android, whose Vulkan surfaces offer no BGRA.
#[cfg(target_vendor = "apple")]
const SURFACE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;
#[cfg(not(target_vendor = "apple"))]
const SURFACE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// Asks for the adapter's own limits: the simulator's Metal falls short of
/// WebGPU's defaults and refuses a device that asks for them.
pub fn open_device() -> Option<DeviceInfo> {
    if device().is_none() {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }))
        .ok()?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            required_features: adapter.features() & wgpu::Features::TIMESTAMP_QUERY,
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .ok()?;
        DEVICE.set(Some(Device {
            instance,
            adapter,
            objects: Vec::new(),
            surfaces: Vec::new(),
            sizes: Vec::new(),
            frames: Vec::new(),
            stamps: None,
            gpu_ms: Vec::new(),
            belt: wgpu::util::StagingBelt::new(device.clone(), BELT_CHUNK_BYTES),
            uploads: None,
            in_flight: Arc::new(AtomicU32::new(0)),
            device,
            queue,
        }));
    }
    let d = device()?;
    Some(DeviceInfo {
        align: d.device.limits().min_uniform_buffer_offset_alignment,
        timestamps: d.device.features().contains(wgpu::Features::TIMESTAMP_QUERY),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BindRefused {
    NoDevice,
    NoSurface,
}

/// # Safety
/// `target` stays valid while the surface is bound: the app unbinds it first.
pub unsafe fn surface_bind(handle: u32, target: wgpu::SurfaceTargetUnsafe) -> Result<(), BindRefused> {
    let d = device().ok_or(BindRefused::NoDevice)?;
    // SAFETY: the caller's target.
    let surface = unsafe { d.instance.create_surface_unsafe(target) }.map_err(|_| BindRefused::NoSurface)?;
    let [width, height] = match d.sizes.get(handle_slot(handle) as usize) {
        Some(&Some((h, size))) if h == handle => size,
        _ => [1, 1],
    };
    let mut config = surface.get_default_config(&d.adapter, width, height).ok_or(BindRefused::NoSurface)?;
    config.format = SURFACE_FORMAT;
    config.present_mode = wgpu::PresentMode::Fifo;
    // Shows through where clear, as the page's premultiplied canvas does. Metal
    // offers only opaque and post-multiplied.
    let modes = surface.get_capabilities(&d.adapter).alpha_modes;
    if let Some(mode) = [wgpu::CompositeAlphaMode::PreMultiplied, wgpu::CompositeAlphaMode::PostMultiplied]
        .into_iter()
        .find(|m| modes.contains(m))
    {
        config.alpha_mode = mode;
    }
    surface.configure(&d.device, &config);
    let slot = handle_slot(handle) as usize;
    if d.surfaces.len() <= slot {
        d.surfaces.resize_with(slot + 1, || None);
    }
    d.surfaces[slot] = Some(Surface { handle, surface, config });
    Ok(())
}

pub fn surface_unbind(handle: u32) {
    if let Some(s) = device().and_then(|d| d.surfaces.get_mut(handle_slot(handle) as usize)) {
        if s.as_ref().is_some_and(|b| b.handle == handle) {
            *s = None;
        }
    }
}

/// # Safety
/// None; unsafe as the page's `install` is.
pub unsafe fn install(_at: u32) -> u32 {
    0
}

pub struct Gpu(());

static GPU: Gpu = Gpu(());

pub fn gpu() -> Option<&'static Gpu> {
    device().map(|_| &GPU)
}

/// The layer keeps three drawables, so with at most two frames unfinished a
/// third is always free and taking it never blocks the JavaScript thread.
const FRAMES_IN_FLIGHT: u32 = 2;

/// A baked mesh, usually the largest write a turn sends, is 1 MiB; larger writes take their own chunk.
const BELT_CHUNK_BYTES: u64 = 1 << 20;

/// False while the GPU holds as many unfinished frames as it may: skip rather than wait.
pub fn frame_room() -> bool {
    device().is_none_or(|d| d.in_flight.load(Ordering::Acquire) < FRAMES_IN_FLIGHT)
}

fn flush(d: &mut Device, then: Option<wgpu::CommandBuffer>) {
    let uploads = d.uploads.take().map(|e| {
        d.belt.finish();
        e.finish()
    });
    let staged = uploads.is_some();
    d.queue.submit(uploads.into_iter().chain(then));
    if staged {
        d.belt.recall();
    }
}

pub fn gpu_publish() {
    if let Some(d) = device() {
        if d.uploads.is_some() {
            flush(d, None);
        }
        if !d.frames.is_empty() {
            for frame in d.frames.drain(..) {
                d.queue.present(frame);
            }
            d.in_flight.fetch_add(1, Ordering::AcqRel);
            let in_flight = Arc::clone(&d.in_flight);
            d.queue.on_submitted_work_done(move || {
                in_flight.fetch_sub(1, Ordering::AcqRel);
            });
        }
        let _ = d.device.poll(wgpu::PollType::Poll);
        read_stamps(d);
    }
}

fn read_stamps(d: &mut Device) {
    let Some(s) = d.stamps.as_mut() else { return };
    for r in &s.reads {
        if r.state.load(Ordering::Acquire) == COPIED {
            r.state.store(MAPPING, Ordering::Release);
            let state = Arc::clone(&r.state);
            r.buffer.slice(..).map_async(wgpu::MapMode::Read, move |result| {
                state.store(if result.is_ok() { MAPPED } else { FREE }, Ordering::Release);
            });
        }
    }
    let period_ns = d.queue.get_timestamp_period() as f64;
    for r in &s.reads {
        if r.state.load(Ordering::Acquire) != MAPPED {
            continue;
        }
        if let Ok(range) = r.buffer.slice(..).get_mapped_range() {
            let (ticks, _) = range.as_chunks::<8>();
            let t: Vec<u64> = ticks.iter().map(|b| u64::from_le_bytes(*b)).collect();
            let mut spans = Vec::with_capacity(t.len() / 2 + 1);
            let (mut first, mut last) = (0u64, 0u64);
            for (k, pair) in t.as_chunks::<2>().0.iter().enumerate() {
                let [b, e] = *pair;
                let ran = r.written & (1 << k) != 0 && e >= b;
                spans.push(if ran { (e - b) as f64 * period_ns / 1e6 } else { 0.0 });
                if ran {
                    first = if first == 0 { b } else { first.min(b) };
                    last = last.max(e);
                }
            }
            spans.push(last.saturating_sub(first) as f64 * period_ns / 1e6);
            d.gpu_ms = spans;
        }
        r.buffer.unmap();
        r.state.store(FREE, Ordering::Release);
    }
}

pub fn room(_bytes: u32) -> bool {
    true
}

/// wgpu takes each call as it is made: there is no range to overrun.
pub fn frame_begin() {}

pub fn frame_commit() -> bool {
    true
}

fn float(word: u32) -> f32 {
    f32::from_bits(word)
}

fn texture_format(code: u32) -> wgpu::TextureFormat {
    match code {
        format::RGBA8_SRGB => wgpu::TextureFormat::Rgba8UnormSrgb,
        format::DEPTH32 => wgpu::TextureFormat::Depth32Float,
        format::RGBA32F => wgpu::TextureFormat::Rgba32Float,
        format::R8 => wgpu::TextureFormat::R8Unorm,
        format::RGBA8 => wgpu::TextureFormat::Rgba8Unorm,
        format::RGBA16F => wgpu::TextureFormat::Rgba16Float,
        _ => SURFACE_FORMAT,
    }
}

fn filter_mode(code: u32) -> wgpu::FilterMode {
    if code == filter::LINEAR {
        wgpu::FilterMode::Linear
    } else {
        wgpu::FilterMode::Nearest
    }
}

fn mipmap_mode(code: u32) -> wgpu::MipmapFilterMode {
    if code == filter::LINEAR {
        wgpu::MipmapFilterMode::Linear
    } else {
        wgpu::MipmapFilterMode::Nearest
    }
}

impl Device {
    fn put(&mut self, id: u32, object: Object) {
        let slot = handle_slot(id) as usize;
        if self.objects.len() <= slot {
            self.objects.resize_with(slot + 1, || None);
        }
        self.objects[slot] = Some(object);
    }

    fn get(&self, id: u32) -> Option<&Object> {
        self.objects.get(handle_slot(id) as usize)?.as_ref()
    }

    fn get_mut(&mut self, id: u32) -> Option<&mut Object> {
        self.objects.get_mut(handle_slot(id) as usize)?.as_mut()
    }

    fn take(&mut self, id: u32) -> Option<Object> {
        self.objects.get_mut(handle_slot(id) as usize)?.take()
    }

    fn buffer(&self, id: u32) -> Option<&wgpu::Buffer> {
        match self.get(id)? {
            Object::Buffer(b) => Some(b),
            _ => None,
        }
    }

    fn view(&self, id: u32) -> Option<&wgpu::TextureView> {
        match self.get(id)? {
            Object::View(v) => Some(v),
            _ => None,
        }
    }

    fn pass(&mut self, id: u32) -> Option<&mut wgpu::RenderPass<'static>> {
        match self.get_mut(id)? {
            Object::Pass(p) => Some(p.as_mut()),
            _ => None,
        }
    }

    fn layout(&self, id: u32) -> Option<&wgpu::BindGroupLayout> {
        match self.get(id)? {
            Object::Layout(l) => Some(l),
            _ => None,
        }
    }
}

fn layout_entries(words: &[u32]) -> Vec<wgpu::BindGroupLayoutEntry> {
    words
        .as_chunks::<3>()
        .0
        .iter()
        .map(|&[binding, stages, kind]| {
            let mut visibility = wgpu::ShaderStages::NONE;
            if stages & record::VERTEX != 0 {
                visibility |= wgpu::ShaderStages::VERTEX;
            }
            if stages & record::FRAGMENT != 0 {
                visibility |= wgpu::ShaderStages::FRAGMENT;
            }
            let buffer = |ty, dynamic| wgpu::BindingType::Buffer {
                ty,
                has_dynamic_offset: dynamic,
                min_binding_size: None,
            };
            let texture = |sample_type, view_dimension| wgpu::BindingType::Texture {
                sample_type,
                view_dimension,
                multisampled: false,
            };
            let ty = match kind {
                record::UNIFORM => buffer(wgpu::BufferBindingType::Uniform, false),
                record::UNIFORM_DYNAMIC => buffer(wgpu::BufferBindingType::Uniform, true),
                record::STORAGE => buffer(wgpu::BufferBindingType::Storage { read_only: true }, false),
                record::TEXTURE => texture(wgpu::TextureSampleType::Float { filterable: true }, wgpu::TextureViewDimension::D2),
                record::TEXTURE_ARRAY => texture(wgpu::TextureSampleType::Float { filterable: true }, wgpu::TextureViewDimension::D2Array),
                record::DEPTH => texture(wgpu::TextureSampleType::Depth, wgpu::TextureViewDimension::D2),
                record::TEXTURE_UNFILTERED => texture(wgpu::TextureSampleType::Float { filterable: false }, wgpu::TextureViewDimension::D2),
                record::SAMPLER_COMPARISON => wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                _ => wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            };
            wgpu::BindGroupLayoutEntry { binding, visibility, ty, count: None }
        })
        .collect()
}

fn topology(code: u32) -> wgpu::PrimitiveTopology {
    match code {
        record::STRIP => wgpu::PrimitiveTopology::TriangleStrip,
        record::LINES => wgpu::PrimitiveTopology::LineList,
        _ => wgpu::PrimitiveTopology::TriangleList,
    }
}

fn compare(code: u32) -> wgpu::CompareFunction {
    match code {
        record::GREATER => wgpu::CompareFunction::Greater,
        record::GREATER_EQUAL => wgpu::CompareFunction::GreaterEqual,
        record::LESS => wgpu::CompareFunction::Less,
        _ => wgpu::CompareFunction::Always,
    }
}

fn vertex_format(code: u32) -> wgpu::VertexFormat {
    match code {
        record::F32X2 => wgpu::VertexFormat::Float32x2,
        record::F32X3 => wgpu::VertexFormat::Float32x3,
        record::F32X4 => wgpu::VertexFormat::Float32x4,
        record::U32X4 => wgpu::VertexFormat::Uint32x4,
        record::U32 => wgpu::VertexFormat::Uint32,
        record::U32X2 => wgpu::VertexFormat::Uint32x2,
        _ => wgpu::VertexFormat::Float32,
    }
}

const ALPHA_BLEND: wgpu::BlendState = wgpu::BlendState {
    color: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::SrcAlpha,
        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
        operation: wgpu::BlendOperation::Add,
    },
    alpha: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
        operation: wgpu::BlendOperation::Add,
    },
};
const ADD_BLEND: wgpu::BlendState = wgpu::BlendState {
    color: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    },
    alpha: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::Zero,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    },
};

impl Gpu {
    pub fn buffer(&self, id: u32, size: u32, usage: u32) {
        let Some(d) = device() else { return };
        let b = d.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: size as u64,
            usage: wgpu::BufferUsages::from_bits_truncate(usage),
            mapped_at_creation: false,
        });
        d.put(id, Object::Buffer(b));
    }

    #[allow(clippy::too_many_arguments)]
    pub fn texture(&self, id: u32, width: u32, height: u32, layers: u32, format: u32, mips: u32, usage: u32) {
        let Some(d) = device() else { return };
        let t = d.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: layers,
            },
            mip_level_count: mips,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: texture_format(format),
            usage: wgpu::TextureUsages::from_bits_truncate(usage),
            view_formats: &[],
        });
        d.put(id, Object::Texture(t));
    }

    /// The image was held before the kernel heard of it, so one the kernel names is there.
    pub fn image_texture(&self, id: u32, image: u32) {
        let Some(d) = device() else { return };
        let images = IMAGES.lock().unwrap_or_else(|e| e.into_inner());
        let Some(image) = images.as_ref().and_then(|m| m.get(&image)) else { return };
        let size = wgpu::Extent3d {
            width: image.width,
            height: image.height,
            depth_or_array_layers: 1,
        };
        let t = d.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        d.queue.write_texture(
            t.as_image_copy(),
            &image.pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(image.width * 4),
                rows_per_image: Some(image.height),
            },
            size,
        );
        drop(images);
        d.put(id, Object::Texture(t));
    }

    #[allow(clippy::too_many_arguments)]
    pub fn view(&self, id: u32, texture: u32, dim: u32, base_mip: u32, mips: u32, base_layer: u32, layers: u32) {
        let Some(d) = device() else { return };
        let Some(Object::Texture(t)) = d.get(texture) else { return };
        let v = t.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(if dim == dimension::D2_ARRAY {
                wgpu::TextureViewDimension::D2Array
            } else {
                wgpu::TextureViewDimension::D2
            }),
            base_mip_level: base_mip,
            mip_level_count: Some(mips),
            base_array_layer: base_layer,
            array_layer_count: Some(layers),
            ..Default::default()
        });
        d.put(id, Object::View(v));
    }

    #[allow(clippy::too_many_arguments)]
    pub fn sampler(&self, id: u32, mag: u32, min: u32, mipmap: u32, addr: u32, cmp: u32, anisotropy: u32) {
        let Some(d) = device() else { return };
        let mode = if addr == address::REPEAT { wgpu::AddressMode::Repeat } else { wgpu::AddressMode::ClampToEdge };
        let s = d.device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: filter_mode(mag),
            min_filter: filter_mode(min),
            mipmap_filter: mipmap_mode(mipmap),
            address_mode_u: mode,
            address_mode_v: mode,
            compare: (cmp == compare::LESS_EQUAL).then_some(wgpu::CompareFunction::LessEqual),
            anisotropy_clamp: anisotropy.max(1) as u16,
            ..Default::default()
        });
        d.put(id, Object::Sampler(s));
    }

    pub fn shader(&self, id: u32, code: &str) {
        let Some(d) = device() else { return };
        let m = d.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(code.into()),
        });
        d.put(id, Object::Shader(m));
    }

    pub fn layout(&self, id: u32, entries: &[u32]) {
        let Some(d) = device() else { return };
        let l = d.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &layout_entries(entries),
        });
        d.put(id, Object::Layout(l));
    }

    pub fn pipeline(&self, id: u32, words: &[u32], layouts: &[u32], module: u32, fmt: u32) {
        let Some(d) = device() else { return };
        let Some(Object::Shader(module)) = d.get(module) else { return };
        let Some(layouts) = layouts.iter().map(|&l| d.layout(l).map(Some)).collect::<Option<Vec<_>>>() else {
            return;
        };
        let mut at = record::HEAD as usize;
        let mut next = || {
            at += 1;
            words[at - 1]
        };
        let buffers: Vec<(u64, wgpu::VertexStepMode, Vec<wgpu::VertexAttribute>)> = (0..next())
            .map(|_| {
                let stride = next() as u64;
                let step = if next() == record::PER_INSTANCE {
                    wgpu::VertexStepMode::Instance
                } else {
                    wgpu::VertexStepMode::Vertex
                };
                let attributes = (0..next())
                    .map(|_| wgpu::VertexAttribute {
                        shader_location: next(),
                        offset: next() as u64,
                        format: vertex_format(next()),
                    })
                    .collect();
                (stride, step, attributes)
            })
            .collect();
        let buffers: Vec<Option<wgpu::VertexBufferLayout>> = buffers
            .iter()
            .map(|(stride, step, attributes)| {
                Some(wgpu::VertexBufferLayout {
                    array_stride: *stride,
                    step_mode: *step,
                    attributes,
                })
            })
            .collect();
        let pipeline_layout = d.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &layouts,
            ..Default::default()
        });
        let blend = match words[8] {
            record::ALPHA => Some(ALPHA_BLEND),
            record::ADD => Some(ADD_BLEND),
            _ => None,
        };
        let targets = [Some(wgpu::ColorTargetState {
            format: texture_format(fmt),
            blend,
            write_mask: wgpu::ColorWrites::ALL,
        })];
        let p = d.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module,
                entry_point: Some(VERTEX_ENTRY[words[record::HEAD as usize - 1] as usize]),
                compilation_options: Default::default(),
                buffers: &buffers,
            },
            primitive: wgpu::PrimitiveState {
                topology: topology(words[2]),
                cull_mode: (words[3] == record::CULL_BACK).then_some(wgpu::Face::Back),
                front_face: wgpu::FrontFace::Ccw,
                ..Default::default()
            },
            depth_stencil: (words[5] != record::NO_DEPTH).then(|| wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(words[4] != 0),
                depth_compare: Some(compare(words[5])),
                stencil: Default::default(),
                bias: wgpu::DepthBiasState {
                    constant: words[6] as i32,
                    slope_scale: float(words[7]),
                    clamp: 0.0,
                },
            }),
            multisample: Default::default(),
            fragment: (words[1] != 0).then_some(wgpu::FragmentState {
                module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &targets,
            }),
            multiview_mask: None,
            cache: None,
        });
        d.put(id, Object::Pipeline(p));
    }

    /// Entries are (binding, resource kind, object, offset, size; 0 the whole buffer).
    pub fn bind_group(&self, id: u32, layout: u32, entries: &[u32]) {
        let Some(d) = device() else { return };
        let Some(layout) = d.layout(layout) else { return };
        let entries: Option<Vec<wgpu::BindGroupEntry>> = entries
            .as_chunks::<5>()
            .0
            .iter()
            .map(|&[binding, kind, object, offset, size]| {
                let resource = match kind {
                    resource::BUFFER => wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: d.buffer(object)?,
                        offset: offset as u64,
                        size: NonZeroU64::new(size as u64),
                    }),
                    resource::VIEW => wgpu::BindingResource::TextureView(d.view(object)?),
                    _ => match d.get(object)? {
                        Object::Sampler(s) => wgpu::BindingResource::Sampler(s),
                        _ => return None,
                    },
                };
                Some(wgpu::BindGroupEntry { binding, resource })
            })
            .collect();
        let Some(entries) = entries else { return };
        let g = d.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout,
            entries: &entries,
        });
        d.put(id, Object::Group(g));
    }

    pub fn destroy(&self, id: u32) {
        if let Some(d) = device() {
            d.take(id);
        }
    }

    pub fn write_buffer(&self, id: u32, offset: u32, bytes: &[u8]) {
        let Some(d) = device() else { return };
        let Some(b) = d.buffer(id).cloned() else { return };
        let (Some(size), true) = (NonZeroU64::new(bytes.len() as u64), offset.is_multiple_of(4) && bytes.len().is_multiple_of(4)) else {
            // Empty spans and spans off the copy alignment go by the queue.
            d.queue.write_buffer(&b, offset as u64, bytes);
            return;
        };
        let encoder = d.uploads.get_or_insert_with(|| d.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default()));
        d.belt.write_buffer(encoder, &b, offset as u64, size).copy_from_slice(bytes);
    }

    pub fn write_texture(&self, id: u32, layer: u32, width: u32, height: u32, bytes: &[u8]) {
        let Some(d) = device() else { return };
        let Some(Object::Texture(texture)) = d.get(id) else { return };
        d.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x: 0, y: 0, z: layer },
                aspect: wgpu::TextureAspect::All,
            },
            bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
    }

    pub fn encoder(&self, id: u32) {
        let Some(d) = device() else { return };
        let e = d.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        d.put(id, Object::Encoder(e));
    }

    #[allow(clippy::too_many_arguments)]
    pub fn copy_texture(&self, encoder: u32, source: u32, x: u32, y: u32, target: u32, tx: u32, ty: u32, layer: u32, width: u32, height: u32) {
        let Some(d) = device() else { return };
        let Some(e) = d.take(encoder) else { return };
        let Object::Encoder(mut e) = e else { return };
        if let (Some(Object::Texture(s)), Some(Object::Texture(t))) = (d.get(source), d.get(target)) {
            e.copy_texture_to_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: s,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x, y, z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyTextureInfo {
                    texture: t,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x: tx, y: ty, z: layer },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
        }
        d.put(encoder, Object::Encoder(e));
    }

    /// A pass whose views are missing is not begun, and every call on it does nothing.
    pub fn pass(&self, id: u32, encoder: u32, desc: &[u32]) {
        let Some(d) = device() else { return };
        let [color, color_load, r, g, b, a, depth, depth_load, depth_clear, query_set, begin, end] = *desc else {
            return;
        };
        let color_view = if color != 0 {
            match d.view(color) {
                Some(v) => Some(v.clone()),
                None => return,
            }
        } else {
            None
        };
        let depth_view = if depth != 0 {
            match d.view(depth) {
                Some(v) => Some(v.clone()),
                None => return,
            }
        } else {
            None
        };
        let color_attachments: Vec<Option<wgpu::RenderPassColorAttachment>> = color_view
            .as_ref()
            .map(|view| wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: if color_load == load::LOAD {
                        wgpu::LoadOp::Load
                    } else {
                        wgpu::LoadOp::Clear(wgpu::Color {
                            r: float(r) as f64,
                            g: float(g) as f64,
                            b: float(b) as f64,
                            a: float(a) as f64,
                        })
                    },
                    store: wgpu::StoreOp::Store,
                },
            })
            .into_iter()
            .map(Some)
            .collect();
        let depth_stencil_attachment = depth_view.as_ref().map(|view| wgpu::RenderPassDepthStencilAttachment {
            view,
            depth_ops: (depth_load != load::READ_ONLY).then_some(wgpu::Operations {
                load: if depth_load == load::LOAD { wgpu::LoadOp::Load } else { wgpu::LoadOp::Clear(float(depth_clear)) },
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        });
        let stamped = match d.get(query_set) {
            Some(Object::QuerySet(set, _)) if query_set != 0 => Some(set.clone()),
            _ => None,
        };
        let timestamp_writes = stamped.as_ref().map(|set| wgpu::RenderPassTimestampWrites {
            query_set: set,
            beginning_of_pass_write_index: (begin != NONE).then_some(begin),
            end_of_pass_write_index: (end != NONE).then_some(end),
        });
        let Some(Object::Encoder(e)) = d.get_mut(encoder) else { return };
        let p = e
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &color_attachments,
                depth_stencil_attachment,
                timestamp_writes,
                occlusion_query_set: None,
                multiview_mask: None,
            })
            .forget_lifetime();
        d.put(id, Object::Pass(Box::new(p)));
    }

    pub fn set_pipeline(&self, pass: u32, pipeline: u32) {
        let Some(d) = device() else { return };
        let Some(Object::Pipeline(p)) = d.get(pipeline) else { return };
        let p = p.clone();
        if let Some(pass) = d.pass(pass) {
            pass.set_pipeline(&p);
        }
    }

    pub fn set_bind_group(&self, pass: u32, index: u32, group: u32, offsets: &[u32]) {
        let Some(d) = device() else { return };
        let Some(Object::Group(g)) = d.get(group) else { return };
        let g = g.clone();
        if let Some(pass) = d.pass(pass) {
            pass.set_bind_group(index, &g, offsets);
        }
    }

    /// A size of 0 is the rest of the buffer.
    pub fn set_vertex_buffer(&self, pass: u32, slot: u32, buffer: u32, offset: u32, size: u32) {
        let Some(d) = device() else { return };
        let Some(b) = d.buffer(buffer) else { return };
        let b = b.clone();
        let range = if size == 0 { b.slice(offset as u64..) } else { b.slice(offset as u64..(offset + size) as u64) };
        if let Some(pass) = d.pass(pass) {
            pass.set_vertex_buffer(slot, range);
        }
    }

    pub fn set_index_buffer(&self, pass: u32, buffer: u32, offset: u32, size: u32) {
        let Some(d) = device() else { return };
        let Some(b) = d.buffer(buffer) else { return };
        let b = b.clone();
        let range = if size == 0 { b.slice(offset as u64..) } else { b.slice(offset as u64..(offset + size) as u64) };
        if let Some(pass) = d.pass(pass) {
            pass.set_index_buffer(range, wgpu::IndexFormat::Uint32);
        }
    }

    pub fn draw(&self, pass: u32, vertices: u32, instances: u32, first_vertex: u32, first_instance: u32) {
        if let Some(pass) = device().and_then(|d| d.pass(pass)) {
            pass.draw(first_vertex..first_vertex + vertices, first_instance..first_instance + instances);
        }
    }

    pub fn draw_indexed(&self, pass: u32, indices: u32, instances: u32, first_index: u32, first_instance: u32) {
        if let Some(pass) = device().and_then(|d| d.pass(pass)) {
            pass.draw_indexed(first_index..first_index + indices, 0, first_instance..first_instance + instances);
        }
    }

    pub fn end(&self, pass: u32) {
        if let Some(d) = device() {
            d.take(pass);
        }
    }

    pub fn submit(&self, encoder: u32) {
        let Some(d) = device() else { return };
        if let Some(Object::Encoder(e)) = d.take(encoder) {
            flush(d, Some(e.finish()));
        }
    }

    /// A surface is configured when it is bound.
    pub fn configure(&self, _surface: u32) {}

    pub fn resize(&self, surface: u32, width: u32, height: u32) {
        let Some(d) = device() else { return };
        let size = [width.max(1), height.max(1)];
        let slot = handle_slot(surface) as usize;
        if d.sizes.len() <= slot {
            d.sizes.resize(slot + 1, None);
        }
        d.sizes[slot] = Some((surface, size));
        let Some(Some(s)) = d.surfaces.get_mut(slot) else { return };
        if s.handle != surface {
            return;
        }
        [s.config.width, s.config.height] = size;
        s.surface.configure(&d.device, &s.config);
    }

    /// None when the surface has no texture to give; the frame's passes into it are not begun.
    pub fn current(&self, surface: u32, view: u32) {
        let Some(d) = device() else { return };
        let Some(Some(s)) = d.surfaces.get_mut(handle_slot(surface) as usize) else { return };
        let frame = match s.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => frame,
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                s.surface.configure(&d.device, &s.config);
                frame
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                s.surface.configure(&d.device, &s.config);
                return;
            }
            _ => {
                return;
            }
        };
        let v = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        d.frames.push(frame);
        d.put(view, Object::View(v));
    }

    /// Timestamps are not asked for on a device (DeviceInfo).
    pub fn query_set(&self, id: u32, count: u32) {
        let Some(d) = device() else { return };
        let set = d.device.create_query_set(&wgpu::QuerySetDescriptor {
            label: None,
            ty: wgpu::QueryType::Timestamp,
            count,
        });
        d.put(id, Object::QuerySet(set, count));
    }

    /// Nothing is read while every read is still on its way.
    pub fn timestamps(&self, encoder: u32, query_set: u32, written: u32) {
        let Some(d) = device() else { return };
        let Some(Object::QuerySet(set, count)) = d.get(query_set) else { return };
        let (set, count) = (set.clone(), *count);
        let bytes = u64::from(count) * 8;
        if d.stamps.as_ref().is_none_or(|s| s.count != count) {
            let buffer = |usage| {
                d.device.create_buffer(&wgpu::BufferDescriptor {
                    label: None,
                    size: bytes,
                    usage,
                    mapped_at_creation: false,
                })
            };
            let read = || StampRead {
                buffer: buffer(wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ),
                state: Arc::new(AtomicU8::new(FREE)),
                written: 0,
            };
            d.stamps = Some(Stamps {
                count,
                resolve: buffer(wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC),
                reads: [read(), read(), read()],
            });
        }
        let Some(Object::Encoder(e)) = d.objects.get_mut(handle_slot(encoder) as usize).and_then(Option::as_mut) else {
            return;
        };
        let Some(s) = d.stamps.as_mut() else { return };
        let Some(r) = s.reads.iter_mut().find(|r| r.state.load(Ordering::Acquire) == FREE) else { return };
        e.resolve_query_set(&set, 0..count, &s.resolve, 0);
        e.copy_buffer_to_buffer(&s.resolve, 0, &r.buffer, 0, Some(bytes));
        r.written = written;
        r.state.store(COPIED, Ordering::Release);
    }

    pub fn gpu_ms(&self, span: u32) -> f64 {
        device().and_then(|d| d.gpu_ms.get(span as usize).copied()).unwrap_or(0.0)
    }
}
