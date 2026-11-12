// xpute-gpu/runtime/run.rs

//! Runs a stream's records on the device, op by op, reading each op's words
//! as spec/gpu/api.json gives them.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Instant;

use xpute_core::abi::handle::handle_slot;
use xpute_runtime::mem::Memory;

use super::make::Make;
use super::name::{flags, of, BUFFER_USAGES, COLOR_WRITES, SHADER_STAGES, TEXTURE_USAGES};
use super::object::Object;
use super::pending::{Pending, ANSWERED, REFUSED, WAITING};
use super::word::WordCursor;
use super::GpuRuntime;
use crate::enums::{
    BUFFER_BINDING_TYPE, CANVAS_ALPHA_MODE, CULL_MODE, LOAD_OP, QUERY_TYPE, STORE_OP,
    TEXTURE_SAMPLE_TYPE,
};
use crate::spec::{op, DEVICE, NONE, STATUS, SURFACE_PREFERRED};

/// Runs `f`, and where `slow` is some and `f` took longer, says `[gpu] <what>
/// took <ms> ms`, the whole milliseconds, as a page's runtime says it
/// (`CoreConfig::slow_ms`, ts/gpu `timed`).
pub(super) fn timed<R>(
    slow: Option<u32>,
    what: impl core::fmt::Display,
    f: impl FnOnce() -> R,
) -> R {
    let Some(slow) = slow else {
        return f();
    };
    let t0 = Instant::now();
    let r = f();
    let ms = t0.elapsed().as_millis();
    if ms > u128::from(slow) {
        xpute_runtime::native::log(&format!("[gpu] {what} took {ms} ms"));
    }
    r
}

/// Hands the stream at `at` back to the core, whether or not it was run:
/// END is stored back to 0 with release, and only where the core published a
/// count (xpute-core io/stream.rs R1, R2). Call it once nothing more of the
/// records is read.
pub fn release(mem: &mut Memory<'_>, at: u32) {
    if mem.load_acquire(at) != 0 {
        mem.store_release(at, 0);
    }
}

impl GpuRuntime {
    /// Runs the stream at `at` as the turn left it, and hands it back once run
    /// (`release`). A frame the stream drew counts as unfinished before the
    /// core can ask to draw another.
    pub fn run(&mut self, mem: &mut Memory<'_>, at: u32) {
        timed(self.slow_ms, "run", || {
            self.execute(mem, at);
            if core::mem::take(&mut self.drew) {
                self.count_frame();
            }
        });
        release(mem, at);
    }

    /// Runs the records of the stream at `at`, laid out as xpute-core
    /// io/stream.rs says, and keeps the stream.
    fn execute(&mut self, mem: &mut Memory<'_>, at: u32) {
        let end = mem.load_acquire(at);
        let head = mem.word(at + 4);
        let first = at + 4 * head;
        let mut k = 0;
        while k + 2 <= end {
            let code = mem.word(first + 4 * k);
            let n = mem.word(first + 4 * (k + 1));
            let mut scratch = core::mem::take(&mut self.scratch);
            scratch.clear();
            let from = (first + 4 * (k + 2)) as usize;
            scratch.extend(
                mem.read(from, 4 * n as usize)
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|w| u32::from_le_bytes(*w)),
            );
            timed(self.slow_ms, format_args!("op {code}"), || {
                self.record(mem, code, &mut WordCursor::new(&scratch))
            });
            self.scratch = scratch;
            k += 2 + n;
        }
        // The turn's staged writes reach the queue with the turn, as a page's
        // runtime submits its own.
        if self.uploads.is_some() {
            self.submit(core::iter::empty());
        }
    }

    /// Stages `bytes` through the belt, a chunk's room at a time, and copies
    /// them into the buffer ahead of what follows. No bytes write nothing, as
    /// in WebGPU.
    pub(super) fn write(&mut self, id: u32, offset: u64, bytes: &[u8]) {
        let buffer = self.buffer(id).clone();
        let mut at = 0;
        while at < bytes.len() {
            let Some((chunk, chunk_at, n)) = self.stage((bytes.len() - at) as u64) else {
                return;
            };
            let piece = &bytes[at..at + n as usize];
            chunk
                .get_mapped_range_mut(chunk_at..chunk_at + n)
                .expect("an upload chunk taken free is mapped")
                .copy_from_slice(piece);
            let encoder = self.uploads.get_or_insert_with(|| {
                self.device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None })
            });
            encoder.copy_buffer_to_buffer(&chunk, chunk_at, &buffer, offset + at as u64, n);
            at += n as usize;
        }
    }

    /// This takes the belt's room for the next `want` bytes of a write. Where
    /// every chunk is taken, it submits the writes so far and waits on the
    /// queue until a chunk is back, as a driver's staging ring waits for the
    /// GPU. It must wait: a write record has no refusal, and its bytes are the
    /// runtime's only while the record runs (xpute-core io/stream.rs R3), so
    /// they are staged before the record returns. It answers none only where
    /// the device is gone and its chunks' mappings were refused; the write is
    /// then lost with everything else the loss ends.
    fn stage(&mut self, want: u64) -> Option<(wgpu::Buffer, u64, u64)> {
        loop {
            if let Some(room) = self.belt.take(want) {
                return Some(room);
            }
            if self.uploads.is_some() {
                self.submit(core::iter::empty());
            }
            if !self.belt.is_returning() {
                return None;
            }
            if self
                .device
                .poll(wgpu::PollType::wait_indefinitely())
                .is_err()
            {
                return None;
            }
        }
    }

    /// Lets the object at `id` go. The writes staged so far reach the queue
    /// first, as a page's runtime submits its own, since a submit refuses a
    /// copy into what has been destroyed.
    pub(super) fn destroy(&mut self, id: u32) {
        self.making.remove(&handle_slot(id));
        let object = self.take(id);
        if matches!(object, Some(Object::Buffer(_) | Object::Texture(_))) && self.uploads.is_some()
        {
            self.submit(core::iter::empty());
        }
        match object {
            Some(Object::Buffer(b)) => b.destroy(),
            Some(Object::Texture(t)) => t.destroy(),
            _ => {}
        }
    }

    /// Submits `then` after the writes staged since the last submit.
    pub(super) fn submit(&mut self, then: impl IntoIterator<Item = wgpu::CommandBuffer>) {
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

    fn texture_place(&self, r: &mut WordCursor) -> OwnedTexture {
        let texture = self.texture(r.u()).clone();
        let mip_level = r.u();
        let origin = wgpu::Origin3d {
            x: r.u(),
            y: r.u(),
            z: r.u(),
        };
        let aspect = of(&self.names.aspect, r.u(), "aspect");
        OwnedTexture {
            texture,
            mip_level,
            origin,
            aspect,
        }
    }

    fn buffer_layout(&self, r: &mut WordCursor) -> OwnedBuffer {
        let buffer = self.buffer(r.u()).clone();
        let offset = u64::from(r.u());
        let layout = wgpu::TexelCopyBufferLayout {
            offset,
            bytes_per_row: Some(r.u()),
            rows_per_image: Some(r.u()),
        };
        OwnedBuffer { buffer, layout }
    }

    fn extent(r: &mut WordCursor) -> wgpu::Extent3d {
        wgpu::Extent3d {
            width: r.u(),
            height: r.u(),
            depth_or_array_layers: r.u(),
        }
    }

    fn timestamps(&self, r: &mut WordCursor) -> Option<(wgpu::QuerySet, Option<u32>, Option<u32>)> {
        let (set, begin, end) = (r.opt(), r.opt(), r.opt());
        set.map(|s| (self.query_set(s).clone(), begin, end))
    }

    fn stencil_face(&self, r: &mut WordCursor) -> wgpu::StencilFaceState {
        wgpu::StencilFaceState {
            compare: of(&self.names.compare, r.u(), "compare"),
            fail_op: of(&self.names.stencil, r.u(), "stencil"),
            depth_fail_op: of(&self.names.stencil, r.u(), "stencil"),
            pass_op: of(&self.names.stencil, r.u(), "stencil"),
        }
    }

    fn record(&mut self, mem: &mut Memory<'_>, code: u32, r: &mut WordCursor) {
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
                let view_formats: Vec<wgpu::TextureFormat> = (0..r.u())
                    .map(|_| of(&self.names.texture_format, r.u(), "format"))
                    .collect();
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
                let dimension = r
                    .opt()
                    .map(|v| of(&self.names.view_dimension, v, "view dimension"));
                let aspect = of(&self.names.aspect, r.u(), "aspect");
                let (base_mip_level, mip_level_count, base_array_layer, array_layer_count) =
                    (r.u(), r.opt(), r.u(), r.opt());
                // Every other field takes WebGPU's default, whatever fields
                // wgpu adds.
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
                let addr = [r.u(), r.u(), r.u()].map(|a| of(&self.names.addr, a, "addr mode"));
                let (mag, min) = (
                    of(&self.names.filter, r.u(), "filter"),
                    of(&self.names.filter, r.u(), "filter"),
                );
                let mip = of(&self.names.mipmap, r.u(), "mipmap filter");
                let (lod_min_clamp, lod_max_clamp) = (r.f32(), r.f32());
                let compare = r.opt().map(|c| of(&self.names.compare, c, "compare"));
                let anisotropy_clamp = r.u() as u16;
                let s = self.device.create_sampler(&wgpu::SamplerDescriptor {
                    label: None,
                    address_mode_u: addr[0],
                    address_mode_v: addr[1],
                    address_mode_w: addr[2],
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
                let m = self
                    .device
                    .create_shader_module(wgpu::ShaderModuleDescriptor {
                        label: None,
                        source: wgpu::ShaderSource::Wgsl(code.into()),
                    });
                self.put(id, Object::Shader(m));
            }
            op::BIND_GROUP_LAYOUT => {
                let id = r.u();
                let entries: Vec<wgpu::BindGroupLayoutEntry> = (0..r.u())
                    .map(|_| {
                        let (binding, visibility, kind, a, b, c) =
                            (r.u(), r.u(), r.u(), r.u(), r.u(), r.u());
                        let ty = match kind {
                            0 => wgpu::BindingType::Buffer {
                                ty: match BUFFER_BINDING_TYPE.get(a as usize).copied() {
                                    Some("uniform") => wgpu::BufferBindingType::Uniform,
                                    Some("storage") => {
                                        wgpu::BufferBindingType::Storage { read_only: false }
                                    }
                                    Some("read-only-storage") => {
                                        wgpu::BufferBindingType::Storage { read_only: true }
                                    }
                                    _ => panic!("[gpu] buffer binding type {a}"),
                                },
                                has_dynamic_offset: b != 0,
                                min_binding_size: core::num::NonZeroU64::new(u64::from(c)),
                            },
                            1 => wgpu::BindingType::Sampler(of(
                                &self.names.sampler_binding,
                                a,
                                "sampler binding type",
                            )),
                            2 => wgpu::BindingType::Texture {
                                sample_type: match TEXTURE_SAMPLE_TYPE.get(a as usize).copied() {
                                    Some("float") => {
                                        wgpu::TextureSampleType::Float { filterable: true }
                                    }
                                    Some("unfilterable-float") => {
                                        wgpu::TextureSampleType::Float { filterable: false }
                                    }
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
                            visibility: flags(
                                &SHADER_STAGES,
                                wgpu::ShaderStages::empty(),
                                visibility,
                            ),
                            ty,
                            count: None,
                        }
                    })
                    .collect();
                let l = self
                    .device
                    .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: None,
                        entries: &entries,
                    });
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
                let p = self
                    .device
                    .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: None,
                        bind_group_layouts: &layouts,
                        immediate_size: 0,
                    });
                self.put(id, Object::PipelineLayout(p));
            }
            op::BIND_GROUP => {
                let id = r.u();
                let layout = r.u();
                let words: Vec<[u32; 5]> = (0..r.u())
                    .map(|_| [r.u(), r.u(), r.u(), r.u(), r.u()])
                    .collect();
                let entries: Vec<wgpu::BindGroupEntry> = words
                    .iter()
                    .map(|&[binding, kind, object, offset, size]| {
                        let resource = match (kind, self.get(object)) {
                            (0, Object::Buffer(b)) => {
                                wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                    buffer: b,
                                    offset: u64::from(offset),
                                    size: Some(size)
                                        .filter(|&s| s != NONE)
                                        .and_then(|s| core::num::NonZeroU64::new(u64::from(s))),
                                })
                            }
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
            op::RENDER_PIPELINE => {
                let (id, make) = self.render_pipeline(r);
                self.put(id, make());
            }
            op::COMPUTE_PIPELINE => {
                let (id, make) = self.compute_pipeline(r);
                self.put(id, make());
            }
            op::RENDER_PIPELINE_ASYNC => {
                let asked = self.render_pipeline(r);
                self.make_later(asked, r);
            }
            op::COMPUTE_PIPELINE_ASYNC => {
                let asked = self.compute_pipeline(r);
                self.make_later(asked, r);
            }
            op::QUERY_SET => {
                let id = r.u();
                let ty = match QUERY_TYPE.get(r.u() as usize).copied() {
                    Some("occlusion") => wgpu::QueryType::Occlusion,
                    Some("timestamp") => wgpu::QueryType::Timestamp,
                    t => panic!("[gpu] query type {t:?}"),
                };
                let q = self.device.create_query_set(&wgpu::QuerySetDescriptor {
                    label: None,
                    ty,
                    count: r.u(),
                });
                self.put(id, Object::QuerySet(q));
            }
            // wgpu names an object only as it makes it, so a later label is
            // dropped.
            op::LABEL => {
                r.u();
                r.text();
            }
            op::DESTROY => {
                let id = r.u();
                self.destroy(id);
            }
            op::WRITE_BUFFER => {
                let (id, offset) = (r.u(), u64::from(r.u()));
                let bytes = r.bytes();
                self.write(id, offset, bytes);
            }
            op::WRITE_BUFFER_AT => {
                let (id, offset, addr, size) =
                    (r.u(), u64::from(r.u()), r.u() as usize, r.u() as usize);
                self.write(id, offset, mem.read(addr, size));
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
                    let (addr, n) = (r.u() as usize, r.u() as usize);
                    self.queue
                        .write_texture(place.info(), mem.read(addr, n), layout, size);
                }
            }
            op::COPY_EXTERNAL_IMAGE => {
                let to = self.texture_place(r);
                let (flip_y, premultiplied) = (r.flag(), r.flag());
                let (addr, n, word, ticket) = (r.u() as usize, r.u() as usize, r.u(), r.u());
                self.copy_external_image(
                    to,
                    mem.read(addr, n),
                    flip_y,
                    premultiplied,
                    word,
                    ticket,
                );
            }
            op::ENCODER => {
                let id = r.u();
                let e = self
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
                self.put(id, Object::Encoder(e));
            }
            op::COPY_BUFFER_TO_BUFFER => {
                let (e, from, from_at, to, to_at, size) = (
                    r.u(),
                    r.u(),
                    u64::from(r.u()),
                    r.u(),
                    u64::from(r.u()),
                    u64::from(r.u()),
                );
                let (from, to) = (self.buffer(from).clone(), self.buffer(to).clone());
                self.encoder(e)
                    .copy_buffer_to_buffer(&from, from_at, &to, to_at, size);
            }
            op::COPY_BUFFER_TO_TEXTURE => {
                let e = r.u();
                let (from, to) = (self.buffer_layout(r), self.texture_place(r));
                let size = Self::extent(r);
                self.encoder(e)
                    .copy_buffer_to_texture(from.info(), to.info(), size);
            }
            op::COPY_TEXTURE_TO_BUFFER => {
                let e = r.u();
                let (from, to) = (self.texture_place(r), self.buffer_layout(r));
                let size = Self::extent(r);
                self.encoder(e)
                    .copy_texture_to_buffer(from.info(), to.info(), size);
            }
            op::COPY_TEXTURE_TO_TEXTURE => {
                let e = r.u();
                let (from, to) = (self.texture_place(r), self.texture_place(r));
                let size = Self::extent(r);
                self.encoder(e)
                    .copy_texture_to_texture(from.info(), to.info(), size);
            }
            op::CLEAR_BUFFER => {
                let (e, b, offset, size) = (r.u(), r.u(), u64::from(r.u()), r.opt().map(u64::from));
                let b = self.buffer(b).clone();
                self.encoder(e).clear_buffer(&b, offset, size);
            }
            op::RESOLVE_QUERY_SET => {
                let (e, q, first, count, b, offset) =
                    (r.u(), r.u(), r.u(), r.u(), r.u(), u64::from(r.u()));
                let (q, b) = (self.query_set(q).clone(), self.buffer(b).clone());
                self.encoder(e)
                    .resolve_query_set(&q, first..first + count, &b, offset);
            }
            op::RENDER_PASS => self.begin_render_pass(r),
            op::COMPUTE_PASS => {
                let (id, e) = (r.u(), r.u());
                let stamps = self.timestamps(r);
                let timestamp_writes =
                    stamps
                        .as_ref()
                        .map(|(q, begin, end)| wgpu::ComputePassTimestampWrites {
                            query_set: q,
                            beginning_of_pass_write_index: *begin,
                            end_of_pass_write_index: *end,
                        });
                let p = self
                    .encoder(e)
                    .begin_compute_pass(&wgpu::ComputePassDescriptor {
                        label: None,
                        timestamp_writes,
                    })
                    .forget_lifetime();
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
                let (pass, slot, b, offset, size) = (
                    r.u(),
                    r.u(),
                    r.u(),
                    u64::from(r.u()),
                    r.opt().map(u64::from),
                );
                let b = self.buffer(b).clone();
                self.render_pass(pass)
                    .set_vertex_buffer(slot, slice(&b, offset, size));
            }
            op::SET_INDEX_BUFFER => {
                let (pass, b) = (r.u(), r.u());
                let format = of(&self.names.index_format, r.u(), "index format");
                let (offset, size) = (u64::from(r.u()), r.opt().map(u64::from));
                let b = self.buffer(b).clone();
                self.render_pass(pass)
                    .set_index_buffer(slice(&b, offset, size), format);
            }
            op::SET_VIEWPORT => {
                let pass = r.u();
                let v = [r.f32(), r.f32(), r.f32(), r.f32(), r.f32(), r.f32()];
                self.render_pass(pass)
                    .set_viewport(v[0], v[1], v[2], v[3], v[4], v[5]);
            }
            op::SET_SCISSOR => {
                let pass = r.u();
                let s = [r.u(), r.u(), r.u(), r.u()];
                self.render_pass(pass)
                    .set_scissor_rect(s[0], s[1], s[2], s[3]);
            }
            op::SET_BLEND_CONSTANT => {
                let pass = r.u();
                let c = [r.f32(), r.f32(), r.f32(), r.f32()].map(f64::from);
                self.render_pass(pass).set_blend_constant(wgpu::Color {
                    r: c[0],
                    g: c[1],
                    b: c[2],
                    a: c[3],
                });
            }
            op::SET_STENCIL_REFERENCE => {
                let pass = r.u();
                let reference = r.u();
                self.render_pass(pass).set_stencil_reference(reference);
            }
            op::DRAW => {
                let (pass, vertices, instances, first_vertex, first_instance) =
                    (r.u(), r.u(), r.u(), r.u(), r.u());
                self.render_pass(pass).draw(
                    first_vertex..first_vertex + vertices,
                    first_instance..first_instance + instances,
                );
            }
            op::DRAW_INDEXED => {
                let (pass, indices, instances, first_index, base_vertex, first_instance) =
                    (r.u(), r.u(), r.u(), r.u(), r.i(), r.u());
                self.render_pass(pass).draw_indexed(
                    first_index..first_index + indices,
                    base_vertex,
                    first_instance..first_instance + instances,
                );
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
                self.compute_pass(pass)
                    .dispatch_workgroups_indirect(&b, offset);
            }
            op::END => {
                let id = r.u();
                drop(self.take(id));
            }
            op::SUBMIT => {
                let mut finished = core::mem::take(&mut self.cmd_bufs);
                finished.extend(r.list().iter().filter_map(|&id| match self.take(id) {
                    Some(Object::Encoder(e)) => Some(e.finish()),
                    _ => None,
                }));
                self.submit(finished.drain(..));
                self.cmd_bufs = finished;
            }
            op::MAP_ASYNC => {
                let (b, offset, size, addr, word, ticket) = (
                    r.u(),
                    u64::from(r.u()),
                    u64::from(r.u()),
                    r.u(),
                    r.u(),
                    r.u(),
                );
                let buffer = self.buffer(b).clone();
                let state = Arc::new(AtomicU8::new(WAITING));
                let answered = Arc::clone(&state);
                buffer
                    .slice(offset..offset + size)
                    .map_async(wgpu::MapMode::Read, move |result| {
                        answered.store(
                            if result.is_ok() { ANSWERED } else { REFUSED },
                            Ordering::Release,
                        );
                    });
                self.pending.push(Pending {
                    state,
                    word,
                    ticket,
                    read: Some((buffer, offset, size, addr)),
                });
            }
            op::QUEUE_DONE => {
                let (word, ticket) = (r.u(), r.u());
                // Staged writes are the queue's work as much as what was
                // submitted, so they are submitted first.
                self.submit(core::iter::empty());
                let state = Arc::new(AtomicU8::new(WAITING));
                let answered = Arc::clone(&state);
                self.queue
                    .on_submitted_work_done(move || answered.store(ANSWERED, Ordering::Release));
                self.pending.push(Pending {
                    state,
                    word,
                    ticket,
                    read: None,
                });
            }
            op::CONFIGURE => {
                let (surface, format, usage, alpha) = (
                    r.u(),
                    r.u(),
                    flags(&TEXTURE_USAGES, wgpu::TextureUsages::empty(), r.u()),
                    r.u(),
                );
                let format = if format == SURFACE_PREFERRED {
                    self.preferred_format
                } else {
                    of(&self.names.texture_format, format, "format")
                };
                let premultiplied = CANVAS_ALPHA_MODE.get(alpha as usize) == Some(&"premultiplied");
                self.surface(surface).cfg = Some((format, usage, premultiplied));
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
                self.drew = true;
            }
            op::DEVICE => {
                let addr = r.u();
                r.u();
                self.block = Some(addr);
                let (limits, bits) = super::open::block(&self.device);
                for (k, w) in limits.into_iter().chain(bits).enumerate() {
                    mem.set_word(addr + 4 * k as u32, w);
                }
                // wgpu resolves timestamps in ticks of this many nanoseconds.
                mem.set_word(
                    addr + 4 * DEVICE::PERIOD_AT as u32,
                    self.queue.get_timestamp_period().to_bits(),
                );
                let preferred = self
                    .names
                    .texture_format
                    .iter()
                    .position(|f| *f == Some(self.preferred_format));
                mem.set_word(
                    addr + 4 * DEVICE::FORMAT_AT as u32,
                    preferred.unwrap_or_else(|| {
                        panic!(
                            "[gpu] {:?} is no format the spec names",
                            self.preferred_format
                        )
                    }) as u32,
                );
                self.tell_counts(mem);
                mem.store_release(addr + 4 * DEVICE::STATUS_AT as u32, STATUS::READY);
            }
            _ => panic!("[gpu] op {code} is no op of spec/gpu/api.json"),
        }
    }

    fn pipeline_layout(&self, layout: Option<u32>) -> Option<wgpu::PipelineLayout> {
        layout.map(|l| match self.get(l) {
            Object::PipelineLayout(l) => l.clone(),
            _ => panic!("[gpu] {l} is no pipeline layout"),
        })
    }

    /// The cache a pipeline the maker compiles goes through, where there is one.
    fn pipeline_cache(&self) -> Option<wgpu::PipelineCache> {
        self.cache.as_ref().map(|c| c.pipelines.clone())
    }

    fn compute_pipeline(&mut self, r: &mut WordCursor) -> (u32, Make) {
        let id = r.u();
        let layout = self.pipeline_layout(r.opt());
        let module = self.shader(r.u()).clone();
        let entry = r.text().to_string();
        let device = self.device.clone();
        let cache = self.pipeline_cache();
        let make: Make = Box::new(move || {
            Object::ComputePipeline(device.create_compute_pipeline(
                &wgpu::ComputePipelineDescriptor {
                    label: None,
                    layout: layout.as_ref(),
                    module: &module,
                    entry_point: Some(&entry),
                    compilation_options: Default::default(),
                    cache: cache.as_ref(),
                },
            ))
        });
        (id, make)
    }

    fn render_pipeline(&mut self, r: &mut WordCursor) -> (u32, Make) {
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
                Layout {
                    stride,
                    step,
                    attributes,
                }
            })
            .collect();
        let topology = of(&self.names.topology, r.u(), "topology");
        let strip_index_format = r
            .opt()
            .map(|s| of(&self.names.index_format, s, "index format"));
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
        let layout = self.pipeline_layout(layout);
        let vertex_module = self.shader(vertex_module).clone();
        let fragment_module = fragment_module.map(|m| self.shader(m).clone());
        let depth_format = depth_format.map(|f| of(&self.names.texture_format, f, "format"));
        let device = self.device.clone();
        let cache = self.pipeline_cache();
        let make: Make = Box::new(move || {
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
            Object::RenderPipeline(
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: None,
                    layout: layout.as_ref(),
                    vertex: wgpu::VertexState {
                        module: &vertex_module,
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
                    depth_stencil: depth_format.map(|format| wgpu::DepthStencilState {
                        format,
                        depth_write_enabled: Some(depth_write_enabled),
                        depth_compare: Some(depth_compare),
                        stencil: wgpu::StencilState {
                            front,
                            back,
                            read_mask,
                            write_mask,
                        },
                        bias,
                    }),
                    multisample,
                    fragment: fragment_module.as_ref().map(|module| wgpu::FragmentState {
                        module,
                        entry_point: Some(&fragment_entry),
                        compilation_options: Default::default(),
                        targets: &targets,
                    }),
                    multiview_mask: None,
                    cache: cache.as_ref(),
                }),
            )
        });
        (id, make)
    }

    fn begin_render_pass(&mut self, r: &mut WordCursor) {
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
            let clear = wgpu::Color {
                r: c[0],
                g: c[1],
                b: c[2],
                a: c[3],
            };
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
        let (dload, dstore, dclear, dro, sload, sstore, sclear, sro) = (
            r.u(),
            r.u(),
            r.f32(),
            r.flag(),
            r.u(),
            r.u(),
            r.u(),
            r.flag(),
        );
        // WebGPU refuses load and store ops on an aspect the format lacks, or
        // on one read only, so those are left unset.
        let depth = depth.map(|v| {
            let (view, format) = self.view(v);
            let (view, has_depth, has_stencil) = (
                view.clone(),
                format.has_depth_aspect(),
                format.has_stencil_aspect(),
            );
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
        let color_attachments: [Option<wgpu::RenderPassColorAttachment>; COLORS] =
            core::array::from_fn(|k| {
                colors[k].as_ref().map(|c| wgpu::RenderPassColorAttachment {
                    view: &c.view,
                    depth_slice: None,
                    resolve_target: c.resolve.as_ref(),
                    ops: c.ops,
                })
            });
        let depth_stencil_attachment = depth.as_ref().map(|(view, depth_ops, stencil_ops)| {
            wgpu::RenderPassDepthStencilAttachment {
                view,
                depth_ops: *depth_ops,
                stencil_ops: *stencil_ops,
            }
        });
        let timestamp_writes =
            stamps
                .as_ref()
                .map(|(q, begin, end)| wgpu::RenderPassTimestampWrites {
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

/// This is the most color attachments a render pass takes: wgpu-hal's
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

/// This is a copy's buffer side, holding its buffer, so that the encoder can be
/// borrowed while the side is in use. `OwnedTexture` is the texture side.
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

pub(super) struct OwnedTexture {
    pub(super) texture: wgpu::Texture,
    mip_level: u32,
    origin: wgpu::Origin3d,
    aspect: wgpu::TextureAspect,
}

impl OwnedTexture {
    pub(super) fn info(&self) -> wgpu::TexelCopyTextureInfo<'_> {
        wgpu::TexelCopyTextureInfo {
            texture: &self.texture,
            mip_level: self.mip_level,
            origin: self.origin,
            aspect: self.aspect,
        }
    }
}
