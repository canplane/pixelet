// xpute-gpu/host/run.rs

//! A stream's records run on the device, op by op (spec/gpu/api.json).

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use xpute_host::mem::Memory;

use super::name::{flags, of, BUFFER_USAGES, COLOR_WRITES, SHADER_STAGES, TEXTURE_USAGES};
use super::object::Object;
use super::pending::{Pending, ANSWERED, REFUSED, WAITING};
use super::word::Words;
use super::GpuHost;
use crate::enums::{BUFFER_BINDING_TYPE, CANVAS_ALPHA_MODE, CULL_MODE, LOAD_OP, QUERY_TYPE, STORE_OP, TEXTURE_SAMPLE_TYPE};
use crate::spec::{op, NONE, STATUS, SURFACE_PREFERRED};

impl GpuHost {
    /// Runs the stream at `at`, `head` words of head, as the turn left it:
    /// word 0 the count of record words past the head, each record its op,
    /// its count of words, then they.
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
            self.record(memory, code, &mut Words::new(&scratch));
            self.scratch = scratch;
            k += 2 + n;
        }
        // The turn's writes reach the queue with it, as the web's writeBuffer does.
        if self.uploads.is_some() {
            self.submit(core::iter::empty());
        }
    }

    /// Nothing for no bytes, as WebGPU writes nothing.
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
                self.copy_external_image(to, memory.read(address, n), flip_y, premultiplied, word, ticket);
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
                let (limits, bits) = super::open::block(&self.device);
                let period_at = limits.len() + bits.len();
                for (k, w) in limits.into_iter().chain(bits).enumerate() {
                    memory.set_word(address + 4 * k as u32, w);
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

/// A copy's sides with their objects held, so the encoder can be borrowed.
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
