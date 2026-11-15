// xpute-gpu-guest/program.test.rs

//! The programs gpu/link/spec/golden/program.tsv records, written with this
//! encoder, and the record's one writer. Each turn ends through `turn` with a
//! request whose ticket is the turn's number, counted from 1; the next turn
//! waits until the host has stored that ticket at the fence, and the last
//! turn reads the result back to the place the record names.
//!
//! Run with `XPUTE_GOLDEN_WRITE=1`, the test encodes the programs, runs each
//! turn on this machine's GPU through xpute-gpu-native-host, and writes the
//! record: the block's layout, each program's turns, its input and the bytes
//! it read back. A command that uses the GPU runs under its device's module
//! (`script/perf/mac.py cargo test …`). Without it the test holds the
//! encoder's turns to the record. An address in a turn is a native pointer on
//! a device, and so differs every run; the record holds each relative to its
//! own block, and each reader finds the address words by the stream spec's
//! ops (xpute-gpu-link `addr::rebase`) and adds its own block's base.

use std::fmt::Write as _;

use xpute_golden::Golden;
use xpute_gpu_link::addr;
use xpute_gpu_link::enums::*;
use xpute_gpu_link::spec::{buffer_usage as B, color_write, shader_stage as S, texture_usage as T};
use xpute_link::mem;

use super::{Buf, Stream};
use crate::encode::*;

type Encoder = Gpu<64>;

/// A `Turn` ends a turn: the test takes the stream's records there.
type Turn<'a> = dyn FnMut(&mut Stream) + 'a;

/// The record, as the readers read it.
const GOLDEN: &str = include_str!("../../../link/spec/golden/program.tsv");

/// The block's size, and where in it the records, a program's input, the
/// fence and its result, and what a program reads back lie.
const BLOCK_SZ: usize = 65536;
const RECS_AT: usize = 16;
const INPUT_AT: usize = 32768;
const DONE_AT: usize = 40960;
const READBACK_AT: usize = 49152;

/// The stream's words: as many as fit before the input.
const STREAM_WORDS: usize = (INPUT_AT - RECS_AT) / 4;

/// The image `sampled_image` copies in: a 4×4 PNG.
const IMAGE: &str = "89504e470d0a1a0a0000000d4948445200000004000000040806000000a9f19e7e0000003649444154789c55ca310d00300805d14f82809a685217adb35644e5302009073091c070cbcb311c3e6c22e3a31f15e8ddd50ed22d0d02f857163979b639650000000049454e44ae426082";

/// The places in the block a program reads and writes.
struct Block {
    base: *mut u8,
    input_len: usize,
    /// How many bytes the program read back, which `read` sets.
    readback_len: core::cell::Cell<usize>,
}

impl Block {
    fn input(&self) -> &'static [u8] {
        // SAFETY: inside the block the test made, which outlives the program.
        unsafe { core::slice::from_raw_parts(self.base.add(INPUT_AT), self.input_len) }
    }

    /// This is the fence and the result word after it (spec RESULT), which only
    /// the host writes.
    fn done(&self) -> &'static [u32; 2] {
        // SAFETY: as `input`; the places do not overlap.
        unsafe { &*self.base.add(DONE_AT).cast::<[u32; 2]>() }
    }

    /// Reads `n` bytes of `buffer` back to the readback place. WebGPU maps
    /// only a buffer made to be mapped, so the bytes are copied into one, and
    /// that one is mapped.
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
        let into = unsafe { core::slice::from_raw_parts_mut(self.base.add(READBACK_AT), n) };
        call::map_async(s, mapped, 0, into, self.done(), ticket);
        self.readback_len.set(n);
    }
}

struct Program {
    name: &'static str,
    encode: fn(&mut Encoder, &mut Stream, &Block, &mut Turn),
    input: Option<&'static str>,
}

const PROGRAMS: [Program; 7] = [
    Program {
        name: "add_arrays",
        encode: add_arrays,
        input: None,
    },
    Program {
        name: "triangle",
        encode: triangle,
        input: None,
    },
    Program {
        name: "indexed_instanced",
        encode: indexed_instanced,
        input: None,
    },
    Program {
        name: "sampled_image",
        encode: sampled_image,
        input: Some(IMAGE),
    },
    Program {
        name: "timestamps",
        encode: timestamps,
        input: None,
    },
    Program {
        name: "dropped_transaction",
        encode: dropped_transaction,
        input: None,
    },
    Program {
        name: "pipelines_async",
        encode: pipelines_async,
        input: None,
    },
];

/// The records the stream holds, taken: the host has them, and gives the
/// buffer back once it has run them.
fn take(s: &mut Stream) -> Vec<u32> {
    let Buf::Free(words) = &mut s.buf else {
        panic!("the stream is out");
    };
    core::mem::take(words)
}

/// A program's turns, each its records with their addresses relative to the
/// block, as the record holds them, and the bytes it read back where `gpu`
/// ran them.
struct Run {
    turns: Vec<Vec<u32>>,
    readback: Vec<u8>,
}

/// Encodes `p` over a fresh block, each turn run on `gpu` where one is given.
fn run(p: &Program, mut gpu: Option<&mut xpute_gpu_native_host::GpuHost>, poll: &dyn Fn()) -> Run {
    let words: &'static mut [u64] = Vec::leak(vec![0; BLOCK_SZ / 8]);
    let base = words.as_mut_ptr().cast::<u8>();
    let input = p.input.map(|h| hex::decode(h).unwrap()).unwrap_or_default();
    // SAFETY: the block just made, the program's alone.
    unsafe { core::slice::from_raw_parts_mut(base.add(INPUT_AT), input.len()) }
        .copy_from_slice(&input);
    let block = Block {
        base,
        input_len: input.len(),
        readback_len: core::cell::Cell::new(0),
    };
    let block_at = mem::addr(&words[0]);
    let mut turns = Vec::new();
    let mut stream = Stream::new(STREAM_WORDS);
    (p.encode)(
        &mut Encoder::new(),
        &mut stream,
        &block,
        &mut |s: &mut Stream| {
            let mut recs = take(s);
            if let Some(host) = gpu.as_deref_mut() {
                let recs_at = block_at + RECS_AT as u64;
                // SAFETY: the block's records' place, the host's until the turn's
                // ticket is at the fence.
                unsafe {
                    mem::write(
                        recs_at,
                        &recs
                            .iter()
                            .flat_map(|w| w.to_le_bytes())
                            .collect::<Vec<_>>(),
                    )
                };
                host.run(recs_at, (recs.len() * 4) as u32);
                let ticket = turns.len() as u32 + 1;
                while mem::load_acquire(block_at + DONE_AT as u64) != ticket {
                    poll();
                    host.poll();
                }
            }
            // The addresses are this run's pointers; the record holds them
            // relative to the block.
            addr::rebase(&mut recs, block_at, 0);
            turns.push(recs);
        },
    );
    // SAFETY: the block, which nothing writes now.
    let readback =
        unsafe { core::slice::from_raw_parts(base.add(READBACK_AT), block.readback_len.get()) }
            .to_vec();
    Run { turns, readback }
}

fn hex_of(words: &[u32]) -> String {
    hex::encode(
        words
            .iter()
            .flat_map(|w| w.to_le_bytes())
            .collect::<Vec<_>>(),
    )
}

/// Writes the record, each program run on this machine's GPU.
fn write() {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("an adapter");
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: wgpu::Features::TIMESTAMP_QUERY,
        ..Default::default()
    }))
    .expect("a device with timestamp queries");
    let mut out = String::new();
    for (k, v) in [
        ("region", BLOCK_SZ),
        ("recs.at", RECS_AT),
        ("input.at", INPUT_AT),
        ("done.at", DONE_AT),
        ("readback.at", READBACK_AT),
    ] {
        let _ = writeln!(out, "{k}\t{v}");
    }
    for (i, p) in PROGRAMS.iter().enumerate() {
        let mut host = xpute_gpu_native_host::GpuHost::new(
            adapter.clone(),
            device.clone(),
            queue.clone(),
            xpute_gpu_native_host::Config {
                preferred_format: wgpu::TextureFormat::Bgra8Unorm,
                configure_at: xpute_gpu_native_host::ConfigureAt::Here,
                frame_latency: 2,
                upload_chunk_sz: 1 << 16,
                present_mode: wgpu::PresentMode::Fifo,
            },
            None,
            || {},
        );
        let r = run(p, Some(&mut host), &|| {
            let _ = device.poll(wgpu::PollType::wait_indefinitely());
        });
        let _ = writeln!(out, "program.{i}\t{}", p.name);
        for (t, recs) in r.turns.iter().enumerate() {
            let _ = writeln!(out, "{}.turn.{t}\t{}", p.name, hex_of(recs));
        }
        if let Some(h) = p.input {
            let _ = writeln!(out, "{}.input\t{h}", p.name);
        }
        let _ = writeln!(out, "{}.readback\t{}", p.name, hex::encode(&r.readback));
    }
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../link/spec/golden/program.tsv"
    );
    std::fs::write(path, out).expect("the record written");
}

/// The encoder writes each program's turns as the record holds them; with
/// `XPUTE_GOLDEN_WRITE=1`, the record is written instead.
#[test]
fn programs_match_record() {
    if std::env::var_os("XPUTE_GOLDEN_WRITE").is_some() {
        return write();
    }
    let g = Golden::load(GOLDEN);
    let mut recorded = Vec::new();
    g.each("program", |k| recorded.push(g.s(k)));
    let names: Vec<&str> = PROGRAMS.iter().map(|p| p.name).collect();
    assert_eq!(names, recorded, "the programs, as the record lists them");
    for p in &PROGRAMS {
        let r = run(p, None, &|| {});
        assert_eq!(
            r.turns.len(),
            g.len(&format!("{}.turn", p.name)),
            "{}'s turns",
            p.name
        );
        for (t, recs) in r.turns.iter().enumerate() {
            let key = format!("{}.turn.{t}", p.name);
            assert_eq!(hex_of(recs), hex::encode(g.bytes(&key)), "{key}");
        }
    }
}

/// This is a target's side in pixels. Its rows are read back 256 bytes apart,
/// the row alignment WebGPU asks of a copy into a buffer.
const SIDE: u32 = 16;

fn le(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|w| w.to_le_bytes()).collect()
}

fn le_f32(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|w| w.to_le_bytes()).collect()
}

fn target(gpu: &mut Encoder, s: &mut Stream) -> (Id, Id) {
    let t = gpu
        .texture(
            s,
            &TextureDescriptor {
                size: [SIDE, SIDE, 1],
                mip_cnt: 1,
                sample_cnt: 1,
                dimension: TextureDimension::_2d,
                format: TextureFormat::Rgba8unorm,
                usage: T::RENDER_ATTACHMENT | T::COPY_SRC,
                view_formats: &[],
            },
        )
        .unwrap();
    let v = gpu.view(s, t, &ViewDescriptor::default()).unwrap();
    (t, v)
}

/// This is one pass drawing into `view`. Each channel of its clear color is a
/// whole number of 255ths, so every device stores the same bytes.
fn pass(gpu: &mut Encoder, s: &mut Stream, e: Id, view: Id) -> Id {
    let colors = [ColorAttachment {
        view,
        resolve: None,
        load: LoadOp::Clear,
        store: StoreOp::Store,
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

/// Copies the target out and reads it back. The pass on `e` must have ended.
fn read_target(gpu: &mut Encoder, s: &mut Stream, block: &Block, e: Id, texture: Id, ticket: u32) {
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
            aspect: TextureAspect::All,
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
    block.read(gpu, s, out, (256 * SIDE) as usize, ticket);
}

fn color_target() -> [ColorTargetState; 1] {
    [ColorTargetState {
        format: TextureFormat::Rgba8unorm,
        blend: None,
        write_mask: color_write::ALL,
    }]
}

const PRIMITIVE: PrimitiveState = PrimitiveState {
    topology: PrimitiveTopology::TriangleList,
    strip_index_format: None,
    front_face: FrontFace::Ccw,
    cull: CullMode::None,
    unclipped_depth: false,
};

/// This computes c = a + b over 256 words.
fn add_arrays(gpu: &mut Encoder, s: &mut Stream, block: &Block, turn: &mut Turn) {
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
                entry(0, BufferBindingType::ReadOnlyStorage),
                entry(1, BufferBindingType::ReadOnlyStorage),
                entry(2, BufferBindingType::Storage),
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
    block.read(gpu, s, c, (4 * N) as usize, 1);
    turn(s);
}

/// This draws one triangle with its corners at whole 32nds. Every pixel center
/// lies at least 0.17 pixels from its edges, so no rasterizer's rounding
/// decides whether a pixel is covered.
fn triangle(gpu: &mut Encoder, s: &mut Stream, block: &Block, turn: &mut Turn) {
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
    read_target(gpu, s, block, e, t, 1);
    turn(s);
}

/// This draws four quads from one indexed list of corners, each instance with
/// its own place and color. Their edges lie on pixel boundaries. The base
/// vertex and the first instance are past 0, so both are put to use.
fn indexed_instanced(gpu: &mut Encoder, s: &mut Stream, block: &Block, turn: &mut Turn) {
    let (t, v) = target(gpu, s);
    // Two vertices come before the quad's corners, and the base vertex skips
    // them.
    let corners = le_f32(&[9.0, 9.0, 9.0, 9.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
    let indices: Vec<u8> = [0u16, 1, 2, 2, 1, 3]
        .iter()
        .flat_map(|i| i.to_le_bytes())
        .collect();
    // These are each instance's offset (two f32) and color (unorm8x4). The
    // first instance index skips the first.
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
        format: VertexFormat::Float32x2,
        offset: 0,
        location: 0,
    }];
    let instance = [
        VertexAttribute {
            format: VertexFormat::Float32x2,
            offset: 0,
            location: 1,
        },
        VertexAttribute {
            format: VertexFormat::Unorm8x4,
            offset: 8,
            location: 2,
        },
    ];
    let buffers = [
        VertexBufferLayout {
            stride: 8,
            step: VertexStepMode::Vertex,
            attributes: &corner,
        },
        VertexBufferLayout {
            stride: 12,
            step: VertexStepMode::Instance,
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
    call::set_index_buffer(s, pass, ib, IndexFormat::Uint16, 0, None);
    call::draw_indexed(s, pass, 6, 4, 0, 2, 1);
    gpu.end(s, pass);
    read_target(gpu, s, block, e, t, 1);
    turn(s);
}

/// This copies a 4×4 PNG into a texture, flipped, in one turn, and draws it
/// over the target in the next. Each pixel samples the nearest texel at a point
/// well inside it, so no rounding picks between two texels.
fn sampled_image(gpu: &mut Encoder, s: &mut Stream, block: &Block, turn: &mut Turn) {
    let image = gpu
        .texture(
            s,
            &TextureDescriptor {
                size: [4, 4, 1],
                mip_cnt: 1,
                sample_cnt: 1,
                dimension: TextureDimension::_2d,
                format: TextureFormat::Rgba8unorm,
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
            aspect: TextureAspect::All,
        },
        true,
        false,
        block.input(),
        block.done(),
        1,
    );
    turn(s);

    let (t, v) = target(gpu, s);
    let iv = gpu.view(s, image, &ViewDescriptor::default()).unwrap();
    let nearest = gpu
        .sampler(
            s,
            &SamplerDescriptor {
                addr: [AddressMode::ClampToEdge; 3],
                mag: FilterMode::Nearest,
                min: FilterMode::Nearest,
                mipmap: MipmapFilterMode::Nearest,
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
                        sample: TextureSampleType::Float,
                        dimension: TextureViewDimension::_2d,
                        multisampled: false,
                    },
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: S::FRAGMENT,
                    layout: BindingLayout::Sampler(SamplerBindingType::Filtering),
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
    read_target(gpu, s, block, e, t, 2);
    turn(s);
}

/// This writes and resolves a pass's begin and end times. Only whether the end
/// is not before the begin is read back, since no device gives the same
/// times twice.
fn timestamps(gpu: &mut Encoder, s: &mut Stream, block: &Block, turn: &mut Turn) {
    let qs = gpu.query_set(s, QueryType::Timestamp, 2).unwrap();
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
                entry(0, BufferBindingType::ReadOnlyStorage),
                entry(1, BufferBindingType::Storage),
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
    block.read(gpu, s, out, 4, 1);
    turn(s);
}

/// A transaction that does not fit is dropped with the objects it made. Its
/// handles are given back and taken again by the next transaction, which
/// writes 7.
fn dropped_transaction(gpu: &mut Encoder, s: &mut Stream, block: &Block, turn: &mut Turn) {
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
                    ty: BufferBindingType::Storage,
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
        // This buffer is kept past the commit. Its handle is one that the
        // dropped transaction gives back and the next takes again.
        let _buf = gpu.buffer(
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
            // These are more bytes than the stream has room for.
            call::write_buffer(s, out, 0, &vec![0; s.left() + 4]);
        }
        gpu.submit(s, &[e]);
        gpu.commit(s)
    };
    assert!(!write(gpu, s, true), "the first transaction does not fit");
    assert!(write(gpu, s, false), "the second does");
    block.read(gpu, s, out, 4, 1);
    turn(s);
}

/// This is the triangle again, with its render pipeline and a compute pipeline
/// made by the async ops. Each is made in a turn of its own, and no record
/// names it until the next turn has found its ticket at the fence.
fn pipelines_async(gpu: &mut Encoder, s: &mut Stream, block: &Block, turn: &mut Turn) {
    let m = gpu
        .shader_module(s, "@compute @workgroup_size(1) fn main() {}\n")
        .unwrap();
    let compute = gpu
        .compute_pipeline_async(
            s,
            &ComputePipelineDescriptor {
                layout: None,
                module: m,
                entry: "main",
            },
            &block.done()[0],
            1,
        )
        .unwrap();
    turn(s);
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
    let render = gpu
        .render_pipeline_async(
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
            &block.done()[0],
            2,
        )
        .unwrap();
    turn(s);
    let e = gpu.encoder(s).unwrap();
    let work = gpu.compute_pass(s, e, &None).unwrap();
    call::set_pipeline(s, work, compute);
    call::dispatch(s, work, [1, 1, 1]);
    gpu.end(s, work);
    let pass = pass(gpu, s, e, v);
    call::set_pipeline(s, pass, render);
    call::draw(s, pass, 3, 1, 0, 0);
    gpu.end(s, pass);
    read_target(gpu, s, block, e, t, 3);
    turn(s);
}
