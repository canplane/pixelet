// xpute-gpu/programs.test.rs

//! The programs spec/golden/gpu/programs.tsv records, as the Rust encoder
//! writes them; cpp/xpute/gpu/encode.test.cpp writes the same calls in C++.
//! Each turn ends through `turn` with a ticket at the done word, the turn's
//! number from 1, which the next turn waits on; the last reads the result
//! back to the record's place.

use xpute_guest::ipc::stream::Stream;

use crate::encode::*;
use crate::enums::*;
use crate::spec::{BUFFER_USAGE as B, COLOR_WRITE, SHADER_STAGE as S, TEXTURE_USAGE as T};

pub type Encoder = Gpu<64>;

/// What ends a turn: the stream published, and whatever runs it.
pub type Turn<'a> = dyn FnMut(&mut Stream) + 'a;

/// Where the record puts what a program reads and writes, in the region
/// that starts at `base`.
pub struct At {
    pub base: *mut u8,
    pub input: usize,
    pub input_len: usize,
    pub done: usize,
    pub readback: usize,
}

impl At {
    fn input(&self) -> &'static [u8] {
        // SAFETY: inside the region the test made, which outlives the program.
        unsafe { core::slice::from_raw_parts(self.base.add(self.input), self.input_len) }
    }

    fn done(&self) -> &'static mut u32 {
        // SAFETY: as `input`; the places do not overlap.
        unsafe { &mut *self.base.add(self.done).cast::<u32>() }
    }

    /// Reads `n` bytes of `buffer` back to the record's place, as WebGPU
    /// reads back: copied into a buffer made to be mapped, then mapped.
    fn read(&self, gpu: &mut Encoder, s: &mut Stream, buffer: Id, n: usize, ticket: u32) {
        let mapped = gpu
            .buffer(
                s,
                &BufferDescriptor {
                    size: n as u32,
                    usage: B::MAP_READ | B::COPY_DST,
                    mapped_at_creation: false,
                },
            )
            .unwrap();
        let e = gpu.encoder(s).unwrap();
        call::copy_buffer_to_buffer(s, e, (buffer, 0), (mapped, 0), n as u32);
        gpu.submit(s, &[e]);
        // SAFETY: as `input`.
        let into = unsafe { core::slice::from_raw_parts_mut(self.base.add(self.readback), n) };
        call::map_async(s, mapped, 0, into, self.done(), ticket);
    }
}

pub struct Program {
    pub name: &'static str,
    pub encode: fn(&mut Encoder, &mut Stream, &At, &mut Turn),
}

pub const PROGRAMS: [Program; 6] = [
    Program {
        name: "add_arrays",
        encode: add_arrays,
    },
    Program {
        name: "triangle",
        encode: triangle,
    },
    Program {
        name: "indexed_instanced",
        encode: indexed_instanced,
    },
    Program {
        name: "sampled_image",
        encode: sampled_image,
    },
    Program {
        name: "timestamps",
        encode: timestamps,
    },
    Program {
        name: "dropped_transaction",
        encode: dropped_transaction,
    },
];

/// A target's side in pixels, its rows read back 256 bytes apart.
pub const SIDE: u32 = 16;

fn le(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|w| w.to_le_bytes()).collect()
}

fn le_f32(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|w| w.to_le_bytes()).collect()
}

/// A 16×16 rgba8unorm target and its view.
fn target(gpu: &mut Encoder, s: &mut Stream) -> (Id, Id) {
    let t = gpu
        .texture(
            s,
            &TextureDescriptor {
                size: [SIDE, SIDE, 1],
                mip_levels: 1,
                samples: 1,
                dimension: TextureDimension::_2d,
                format: TextureFormat::rgba8unorm,
                usage: T::RENDER_ATTACHMENT | T::COPY_SRC,
                view_formats: &[],
            },
        )
        .unwrap();
    let v = gpu.view(s, t, &ViewDescriptor::default()).unwrap();
    (t, v)
}

/// One pass drawing into `view`, cleared to a colour each channel of which
/// is a whole number of 255ths.
fn pass(gpu: &mut Encoder, s: &mut Stream, e: Id, view: Id) -> Id {
    let colors = [ColorAttachment {
        view,
        resolve: None,
        load: LoadOp::clear,
        store: StoreOp::store,
        clear: [0.0, 0.0, 0.4, 1.0],
    }];
    gpu.render_pass(
        s,
        e,
        &RenderPassDescriptor {
            colors: &colors,
            depth_stencil: None,
            timestamps: None,
            occlusion: None,
        },
    )
    .unwrap()
}

/// The target copied out and read back, after the pass on `e` has ended.
fn read_target(gpu: &mut Encoder, s: &mut Stream, at: &At, e: Id, texture: Id, ticket: u32) {
    let out = gpu
        .buffer(
            s,
            &BufferDescriptor {
                size: 256 * SIDE,
                usage: B::COPY_DST | B::COPY_SRC,
                mapped_at_creation: false,
            },
        )
        .unwrap();
    call::copy_texture_to_buffer(
        s,
        e,
        &TexelCopyTexture {
            texture,
            mip: 0,
            origin: [0, 0, 0],
            aspect: TextureAspect::all,
        },
        &TexelCopyBuffer {
            buffer: out,
            offset: 0,
            bytes_per_row: 256,
            rows_per_image: SIDE,
        },
        [SIDE, SIDE, 1],
    );
    gpu.submit(s, &[e]);
    at.read(gpu, s, out, (256 * SIDE) as usize, ticket);
}

fn color_target() -> [ColorTargetState; 1] {
    [ColorTargetState {
        format: TextureFormat::rgba8unorm,
        blend: None,
        write_mask: COLOR_WRITE::ALL,
    }]
}

const PRIMITIVE: PrimitiveState = PrimitiveState {
    topology: PrimitiveTopology::triangle_list,
    strip_index_format: None,
    front_face: FrontFace::ccw,
    cull: CullMode::none,
    unclipped_depth: false,
};

/// c = a + b over 256 words.
fn add_arrays(gpu: &mut Encoder, s: &mut Stream, at: &At, turn: &mut Turn) {
    const N: u32 = 256;
    let storage = |gpu: &mut Encoder, s: &mut Stream| {
        gpu.buffer(
            s,
            &BufferDescriptor {
                size: 4 * N,
                usage: B::STORAGE | B::COPY_DST | B::COPY_SRC,
                mapped_at_creation: false,
            },
        )
        .unwrap()
    };
    let (a, b, c) = (storage(gpu, s), storage(gpu, s), storage(gpu, s));
    call::write_buffer(s, a, 0, &le(&(0..N).collect::<Vec<_>>()));
    call::write_buffer(s, b, 0, &le(&(0..N).map(|i| 3 * i + 7).collect::<Vec<_>>()));
    let m = gpu
        .shader_module(
            s,
            "@group(0) @binding(0) var<storage, read> a: array<u32>;\n\
             @group(0) @binding(1) var<storage, read> b: array<u32>;\n\
             @group(0) @binding(2) var<storage, read_write> c: array<u32>;\n\
             @compute @workgroup_size(64) fn main(@builtin(global_invocation_id) i: vec3u) { c[i.x] = a[i.x] + b[i.x]; }\n",
        )
        .unwrap();
    let entry = |binding, ty| BindGroupLayoutEntry {
        binding,
        visibility: S::COMPUTE,
        layout: BindingLayout::Buffer {
            ty,
            dynamic_offset: false,
            min_binding_size: 0,
        },
    };
    let bgl = gpu
        .bind_group_layout(
            s,
            &[
                entry(0, BufferBindingType::read_only_storage),
                entry(1, BufferBindingType::read_only_storage),
                entry(2, BufferBindingType::storage),
            ],
        )
        .unwrap();
    let pl = gpu.pipeline_layout(s, &[bgl]).unwrap();
    let res = |binding, buffer| BindGroupEntry {
        binding,
        resource: Resource::Buffer {
            buffer,
            offset: 0,
            size: None,
        },
    };
    let bg = gpu
        .bind_group(s, bgl, &[res(0, a), res(1, b), res(2, c)])
        .unwrap();
    let p = gpu
        .compute_pipeline(
            s,
            &ComputePipelineDescriptor {
                layout: Some(pl),
                module: m,
                entry: "main",
            },
        )
        .unwrap();
    let e = gpu.encoder(s).unwrap();
    let pass = gpu.compute_pass(s, e, &None).unwrap();
    call::set_pipeline(s, pass, p);
    call::set_bind_group(s, pass, 0, bg, &[]);
    call::dispatch(s, pass, [N / 64, 1, 1]);
    gpu.end(s, pass);
    gpu.submit(s, &[e]);
    at.read(gpu, s, c, (4 * N) as usize, 1);
    turn(s);
}

/// One triangle whose corners are whole 32nds, every pixel centre at least
/// 0.17 pixels from its edges, so no rasterizer's rounding decides one.
fn triangle(gpu: &mut Encoder, s: &mut Stream, at: &At, turn: &mut Turn) {
    let (t, v) = target(gpu, s);
    let m = gpu
        .shader_module(
            s,
            "@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {\n\
             \x20 var p = array(vec2f(-0.75, -0.78125), vec2f(0.9375, -0.71875), vec2f(-0.75, 0.96875));\n\
             \x20 return vec4f(p[i], 0.0, 1.0);\n\
             }\n\
             @fragment fn fs() -> @location(0) vec4f { return vec4f(1.0, 0.2, 0.0, 1.0); }\n",
        )
        .unwrap();
    let targets = color_target();
    let p = gpu
        .render_pipeline(
            s,
            &RenderPipelineDescriptor {
                layout: None,
                vertex: VertexState {
                    module: m,
                    entry: "vs",
                    buffers: &[],
                },
                primitive: PRIMITIVE,
                depth_stencil: None,
                multisample: MULTISAMPLE_NONE,
                fragment: Some(FragmentState {
                    module: m,
                    entry: "fs",
                    targets: &targets,
                }),
            },
        )
        .unwrap();
    let e = gpu.encoder(s).unwrap();
    let pass = pass(gpu, s, e, v);
    call::set_pipeline(s, pass, p);
    call::draw(s, pass, 3, 1, 0, 0);
    gpu.end(s, pass);
    read_target(gpu, s, at, e, t, 1);
    turn(s);
}

/// Four quads of one indexed corner list, each instance its place and
/// colour, edges on pixel boundaries; base vertex and first instance past 0.
fn indexed_instanced(gpu: &mut Encoder, s: &mut Stream, at: &At, turn: &mut Turn) {
    let (t, v) = target(gpu, s);
    // Two vertices before the quad's, which the base vertex skips.
    let corners = le_f32(&[9.0, 9.0, 9.0, 9.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
    let indices: Vec<u8> = [0u16, 1, 2, 2, 1, 3]
        .iter()
        .flat_map(|i| i.to_le_bytes())
        .collect();
    // Each: its offset (two f32), its colour (unorm8x4); the first is skipped.
    let places: [([f32; 2], [u8; 4]); 5] = [
        ([0.0, 0.0], [1, 2, 3, 4]),
        ([-1.0, -1.0], [255, 0, 0, 255]),
        ([-0.5, -0.25], [0, 255, 0, 255]),
        ([0.25, -0.75], [10, 20, 30, 255]),
        ([0.375, 0.5], [200, 100, 50, 255]),
    ];
    let instances: Vec<u8> = places
        .iter()
        .flat_map(|(o, c)| le_f32(o).into_iter().chain(c.iter().copied()))
        .collect();
    let buffer = |gpu: &mut Encoder, s: &mut Stream, usage, bytes: &[u8]| {
        let b = gpu
            .buffer(
                s,
                &BufferDescriptor {
                    size: bytes.len() as u32,
                    usage: usage | B::COPY_DST,
                    mapped_at_creation: false,
                },
            )
            .unwrap();
        call::write_buffer(s, b, 0, bytes);
        b
    };
    let (vb, ib, inst) = (
        buffer(gpu, s, B::VERTEX, &corners),
        buffer(gpu, s, B::INDEX, &indices),
        buffer(gpu, s, B::VERTEX, &instances),
    );
    let m = gpu
        .shader_module(
            s,
            "struct O { @builtin(position) p: vec4f, @location(0) @interpolate(flat) c: vec4f }\n\
             @vertex fn vs(@location(0) corner: vec2f, @location(1) offset: vec2f, @location(2) color: vec4f) -> O {\n\
             \x20 return O(vec4f(offset + corner * 0.5, 0.0, 1.0), color);\n\
             }\n\
             @fragment fn fs(i: O) -> @location(0) vec4f { return i.c; }\n",
        )
        .unwrap();
    let corner = [VertexAttribute {
        format: VertexFormat::float32x2,
        offset: 0,
        location: 0,
    }];
    let instance = [
        VertexAttribute {
            format: VertexFormat::float32x2,
            offset: 0,
            location: 1,
        },
        VertexAttribute {
            format: VertexFormat::unorm8x4,
            offset: 8,
            location: 2,
        },
    ];
    let buffers = [
        VertexBufferLayout {
            stride: 8,
            step: VertexStepMode::vertex,
            attributes: &corner,
        },
        VertexBufferLayout {
            stride: 12,
            step: VertexStepMode::instance,
            attributes: &instance,
        },
    ];
    let targets = color_target();
    let p = gpu
        .render_pipeline(
            s,
            &RenderPipelineDescriptor {
                layout: None,
                vertex: VertexState {
                    module: m,
                    entry: "vs",
                    buffers: &buffers,
                },
                primitive: PRIMITIVE,
                depth_stencil: None,
                multisample: MULTISAMPLE_NONE,
                fragment: Some(FragmentState {
                    module: m,
                    entry: "fs",
                    targets: &targets,
                }),
            },
        )
        .unwrap();
    let e = gpu.encoder(s).unwrap();
    let pass = pass(gpu, s, e, v);
    call::set_pipeline(s, pass, p);
    call::set_vertex_buffer(s, pass, 0, vb, 0, None);
    call::set_vertex_buffer(s, pass, 1, inst, 0, None);
    call::set_index_buffer(s, pass, ib, IndexFormat::uint16, 0, None);
    call::draw_indexed(s, pass, 6, 4, 0, 2, 1);
    gpu.end(s, pass);
    read_target(gpu, s, at, e, t, 1);
    turn(s);
}

/// A 4×4 PNG copied in one turn, flipped, and drawn the next over the
/// target, each pixel sampled nearest at a texel's inside.
fn sampled_image(gpu: &mut Encoder, s: &mut Stream, at: &At, turn: &mut Turn) {
    let image = gpu
        .texture(
            s,
            &TextureDescriptor {
                size: [4, 4, 1],
                mip_levels: 1,
                samples: 1,
                dimension: TextureDimension::_2d,
                format: TextureFormat::rgba8unorm,
                usage: T::TEXTURE_BINDING | T::COPY_DST | T::RENDER_ATTACHMENT,
                view_formats: &[],
            },
        )
        .unwrap();
    call::copy_external_image(
        s,
        &TexelCopyTexture {
            texture: image,
            mip: 0,
            origin: [0, 0, 0],
            aspect: TextureAspect::all,
        },
        true,
        false,
        at.input(),
        at.done(),
        1,
    );
    turn(s);

    let (t, v) = target(gpu, s);
    let iv = gpu.view(s, image, &ViewDescriptor::default()).unwrap();
    let nearest = gpu
        .sampler(
            s,
            &SamplerDescriptor {
                address: [AddressMode::clamp_to_edge; 3],
                mag: FilterMode::nearest,
                min: FilterMode::nearest,
                mipmap: MipmapFilterMode::nearest,
                lod: [0.0, 32.0],
                compare: None,
                max_anisotropy: 1,
            },
        )
        .unwrap();
    let bgl = gpu
        .bind_group_layout(
            s,
            &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: S::FRAGMENT,
                    layout: BindingLayout::Texture {
                        sample: TextureSampleType::float,
                        dimension: TextureViewDimension::_2d,
                        multisampled: false,
                    },
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: S::FRAGMENT,
                    layout: BindingLayout::Sampler(SamplerBindingType::filtering),
                },
            ],
        )
        .unwrap();
    let pl = gpu.pipeline_layout(s, &[bgl]).unwrap();
    let bg = gpu
        .bind_group(
            s,
            bgl,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: Resource::View(iv),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: Resource::Sampler(nearest),
                },
            ],
        )
        .unwrap();
    let m = gpu
        .shader_module(
            s,
            "@group(0) @binding(0) var image: texture_2d<f32>;\n\
             @group(0) @binding(1) var nearest: sampler;\n\
             @vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {\n\
             \x20 var p = array(vec2f(-1.0, -1.0), vec2f(3.0, -1.0), vec2f(-1.0, 3.0));\n\
             \x20 return vec4f(p[i], 0.0, 1.0);\n\
             }\n\
             @fragment fn fs(@builtin(position) p: vec4f) -> @location(0) vec4f { return textureSample(image, nearest, p.xy / 16.0); }\n",
        )
        .unwrap();
    let targets = color_target();
    let p = gpu
        .render_pipeline(
            s,
            &RenderPipelineDescriptor {
                layout: Some(pl),
                vertex: VertexState {
                    module: m,
                    entry: "vs",
                    buffers: &[],
                },
                primitive: PRIMITIVE,
                depth_stencil: None,
                multisample: MULTISAMPLE_NONE,
                fragment: Some(FragmentState {
                    module: m,
                    entry: "fs",
                    targets: &targets,
                }),
            },
        )
        .unwrap();
    let e = gpu.encoder(s).unwrap();
    let pass = pass(gpu, s, e, v);
    call::set_pipeline(s, pass, p);
    call::set_bind_group(s, pass, 0, bg, &[]);
    call::draw(s, pass, 3, 1, 0, 0);
    gpu.end(s, pass);
    read_target(gpu, s, at, e, t, 2);
    turn(s);
}

/// A pass's begin and end written, resolved, and whether the end is not
/// before the begin read back: the times themselves are no device's twice.
fn timestamps(gpu: &mut Encoder, s: &mut Stream, at: &At, turn: &mut Turn) {
    let qs = gpu.query_set(s, QueryType::timestamp, 2).unwrap();
    let times = gpu
        .buffer(
            s,
            &BufferDescriptor {
                size: 16,
                usage: B::QUERY_RESOLVE | B::STORAGE,
                mapped_at_creation: false,
            },
        )
        .unwrap();
    let out = gpu
        .buffer(
            s,
            &BufferDescriptor {
                size: 4,
                usage: B::STORAGE | B::COPY_SRC,
                mapped_at_creation: false,
            },
        )
        .unwrap();
    let m = gpu
        .shader_module(
            s,
            "@group(0) @binding(0) var<storage, read> t: array<vec2u, 2>;\n\
             @group(0) @binding(1) var<storage, read_write> o: array<u32>;\n\
             @compute @workgroup_size(1) fn idle() {}\n\
             @compute @workgroup_size(1) fn check() { o[0] = u32(t[1].y > t[0].y || (t[1].y == t[0].y && t[1].x >= t[0].x)); }\n",
        )
        .unwrap();
    let entry = |binding, ty| BindGroupLayoutEntry {
        binding,
        visibility: S::COMPUTE,
        layout: BindingLayout::Buffer {
            ty,
            dynamic_offset: false,
            min_binding_size: 0,
        },
    };
    let bgl = gpu
        .bind_group_layout(
            s,
            &[
                entry(0, BufferBindingType::read_only_storage),
                entry(1, BufferBindingType::storage),
            ],
        )
        .unwrap();
    let pl = gpu.pipeline_layout(s, &[bgl]).unwrap();
    let res = |binding, buffer| BindGroupEntry {
        binding,
        resource: Resource::Buffer {
            buffer,
            offset: 0,
            size: None,
        },
    };
    let bg = gpu
        .bind_group(s, bgl, &[res(0, times), res(1, out)])
        .unwrap();
    let idle = gpu
        .compute_pipeline(
            s,
            &ComputePipelineDescriptor {
                layout: Some(pl),
                module: m,
                entry: "idle",
            },
        )
        .unwrap();
    let check = gpu
        .compute_pipeline(
            s,
            &ComputePipelineDescriptor {
                layout: Some(pl),
                module: m,
                entry: "check",
            },
        )
        .unwrap();
    let e = gpu.encoder(s).unwrap();
    let timed = gpu
        .compute_pass(
            s,
            e,
            &Some(TimestampWrites {
                query_set: qs,
                begin: Some(0),
                end: Some(1),
            }),
        )
        .unwrap();
    call::set_pipeline(s, timed, idle);
    call::set_bind_group(s, timed, 0, bg, &[]);
    call::dispatch(s, timed, [1, 1, 1]);
    gpu.end(s, timed);
    call::resolve_query_set(s, e, qs, 0, 2, times, 0);
    let pass = gpu.compute_pass(s, e, &None).unwrap();
    call::set_pipeline(s, pass, check);
    call::set_bind_group(s, pass, 0, bg, &[]);
    call::dispatch(s, pass, [1, 1, 1]);
    gpu.end(s, pass);
    gpu.submit(s, &[e]);
    at.read(gpu, s, out, 4, 1);
    turn(s);
}

/// A transaction that does not fit dropped with the objects it made, its
/// handles given back and taken again by the next one, which writes 7.
fn dropped_transaction(gpu: &mut Encoder, s: &mut Stream, at: &At, turn: &mut Turn) {
    let out = gpu
        .buffer(
            s,
            &BufferDescriptor {
                size: 4,
                usage: B::STORAGE | B::COPY_SRC,
                mapped_at_creation: false,
            },
        )
        .unwrap();
    let m = gpu
        .shader_module(
            s,
            "@group(0) @binding(0) var<storage, read_write> o: array<u32>;\n@compute @workgroup_size(1) fn main() { o[0] = 7u; }\n",
        )
        .unwrap();
    let bgl = gpu
        .bind_group_layout(
            s,
            &[BindGroupLayoutEntry {
                binding: 0,
                visibility: S::COMPUTE,
                layout: BindingLayout::Buffer {
                    ty: BufferBindingType::storage,
                    dynamic_offset: false,
                    min_binding_size: 0,
                },
            }],
        )
        .unwrap();
    let pl = gpu.pipeline_layout(s, &[bgl]).unwrap();
    let bg = gpu
        .bind_group(
            s,
            bgl,
            &[BindGroupEntry {
                binding: 0,
                resource: Resource::Buffer {
                    buffer: out,
                    offset: 0,
                    size: None,
                },
            }],
        )
        .unwrap();
    let p = gpu
        .compute_pipeline(
            s,
            &ComputePipelineDescriptor {
                layout: Some(pl),
                module: m,
                entry: "main",
            },
        )
        .unwrap();
    let write = |gpu: &mut Encoder, s: &mut Stream, spill: bool| {
        gpu.begin(s);
        // Held past the commit: the handle a dropped one gives back.
        let _held = gpu.buffer(
            s,
            &BufferDescriptor {
                size: 4,
                usage: B::STORAGE,
                mapped_at_creation: false,
            },
        );
        let e = gpu.encoder(s).unwrap();
        let pass = gpu.compute_pass(s, e, &None).unwrap();
        call::set_pipeline(s, pass, p);
        call::set_bind_group(s, pass, 0, bg, &[]);
        call::dispatch(s, pass, [1, 1, 1]);
        gpu.end(s, pass);
        if spill {
            // More than the stream holds.
            call::write_buffer(s, out, 0, &vec![0; s.left() + 4]);
        }
        gpu.submit(s, &[e]);
        gpu.commit(s)
    };
    assert!(!write(gpu, s, true), "the first transaction does not fit");
    assert!(write(gpu, s, false), "the second does");
    at.read(gpu, s, out, 4, 1);
    turn(s);
}
