//! Runs the WGSL rust-gpu made of the caves' path (cave.wgsl) on the GPU
//! over a tile's nodes, X across the wrap and every axis across zero, and
//! diffs each node's bits against pxl-spec's own `Cave` on the CPU.

use pxl_spec::procedural::Cave;
use wgpu::util::DeviceExt;

const ACROSS: [u32; 3] = [8, 8, 8];

/// pxl-spec's answer, in the shader's bits: 0 hollow, 1 whole known, 2 whole's value.
fn cpu(zoom: u8, c: [i64; 3]) -> u32 {
    let cave = Cave::of(zoom, c);
    let whole = cave.whole(zoom);
    u32::from(cave.hollow()) | (u32::from(whole.is_some()) | u32::from(whole == Some(true)) << 1) << 1
}

fn main() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).expect("adapter");
    let info = adapter.get_info();
    println!("adapter: {} ({:?})", info.name, info.backend);
    let has = adapter.features();
    println!("SHADER_INT64: {}, SHADER_F64: {}", has.contains(wgpu::Features::SHADER_INT64), has.contains(wgpu::Features::SHADER_F64));
    if !has.contains(wgpu::Features::SHADER_INT64) {
        println!("no SHADER_INT64 on this adapter: the u64 wall, at run time");
        return;
    }
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: wgpu::Features::SHADER_INT64,
        ..Default::default()
    }))
    .expect("device");
    let file = std::env::args().nth(1).unwrap_or_else(|| "cave.wgsl".into());
    let source = std::fs::read_to_string(format!("{}/../{file}", env!("CARGO_MANIFEST_DIR"))).expect("wgsl");
    let head_only = file.contains("head");
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("cave"),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let pipeline_of = |entry: &str| {
        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: None,
            module: &module,
            entry_point: Some(entry),
            compilation_options: Default::default(),
            cache: None,
        })
    };
    let pipeline = pipeline_of("main_cs");
    let seeds = pipeline_of("seed_cs");
    let n = (ACROSS[0] * ACROSS[1] * ACROSS[2]) as usize;
    let (mut asked, mut differ) = (0usize, 0usize);
    for zoom in [19u8, 20, 22, 24] {
        let half = 1i64 << (zoom - 1);
        for x0 in [half - 4, -half - 4] {
            let first = [x0, -4, -4];
            let mut ask = [0u32; 8];
            ask[0] = u32::from(zoom);
            for k in 0..3 {
                ask[1 + k] = first[k] as i32 as u32;
                ask[4 + k] = ACROSS[k];
            }
            let ask_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: &ask.iter().flat_map(|w| w.to_le_bytes()).collect::<Vec<u8>>(),
                usage: wgpu::BufferUsages::STORAGE,
            });
            let out = device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: (n * 4) as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            let read = device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: (n * 4) as u64,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &pipeline.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: ask_buf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: out.as_entire_binding() },
                ],
            });
            let mut enc = device.create_command_encoder(&Default::default());
            {
                let mut pass = enc.begin_compute_pass(&Default::default());
                pass.set_pipeline(&pipeline);
                pass.set_bind_group(0, &bind, &[]);
                pass.dispatch_workgroups(ACROSS[0] / 4, ACROSS[1] / 4, ACROSS[2] / 4);
            }
            enc.copy_buffer_to_buffer(&out, 0, &read, 0, (n * 4) as u64);
            queue.submit([enc.finish()]);
            read.slice(..).map_async(wgpu::MapMode::Read, |r| r.expect("map"));
            device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
            let gpu: Vec<u32> = read.slice(..).get_mapped_range().expect("mapped").chunks(4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
            let (mut hollow, mut here, mut probe_differ, mut probe_gpu) = (0, 0, 0, 0);
            for z in 0..ACROSS[2] {
                for y in 0..ACROSS[1] {
                    for x in 0..ACROSS[0] {
                        let i = (x + ACROSS[0] * (y + ACROSS[1] * z)) as usize;
                        let c = [first[0] + i64::from(x), first[1] + i64::from(y), first[2] + i64::from(z)];
                        let want = cpu(zoom, c);
                        let probe = gpu_query_shader::answer(&gpu_query_shader::Ask { zoom: u32::from(zoom), first: [first[0] as i32, first[1] as i32, first[2] as i32], across: ACROSS }, [x, y, z]);
                        probe_differ += usize::from(probe != want);
                        probe_gpu += usize::from(probe != gpu[i]);
                        hollow += (want & 1) as usize;
                        if gpu[i] != want {
                            here += 1;
                            if here <= 3 {
                                println!("  differ z{zoom} {c:?}: gpu {:03b} cpu {:03b}", gpu[i], want);
                            }
                        }
                    }
                }
            }
            // The hash alone: seed_cs over the same nodes, two words a node.
            let out2 = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: (n * 8) as u64, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC, mapped_at_creation: false });
            let read2 = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: (n * 8) as u64, usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
            let bind2 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &seeds.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: ask_buf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: out2.as_entire_binding() },
                ],
            });
            let mut enc = device.create_command_encoder(&Default::default());
            {
                let mut pass = enc.begin_compute_pass(&Default::default());
                pass.set_pipeline(&seeds);
                pass.set_bind_group(0, &bind2, &[]);
                pass.dispatch_workgroups(ACROSS[0] / 4, ACROSS[1] / 4, ACROSS[2] / 4);
            }
            enc.copy_buffer_to_buffer(&out2, 0, &read2, 0, (n * 8) as u64);
            queue.submit([enc.finish()]);
            read2.slice(..).map_async(wgpu::MapMode::Read, |r| r.expect("map"));
            device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
            let words: Vec<u32> = read2.slice(..).get_mapped_range().expect("mapped").chunks(4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
            let mut seed_differ = 0;
            for z in 0..ACROSS[2] {
                for y in 0..ACROSS[1] {
                    for x in 0..ACROSS[0] {
                        let i = (x + ACROSS[0] * (y + ACROSS[1] * z)) as usize;
                        let c = [first[0] + i64::from(x), first[1] + i64::from(y), first[2] + i64::from(z)];
                        let want = if head_only {
                            let m = |x: u64| {
                                let mut z = x.wrapping_add(0x9e3779b97f4a7c15);
                                z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
                                z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
                                z ^ (z >> 31)
                            };
                            m(m(0xd0c0dec05eed0001 ^ 0xca7e_0001) ^ u64::from(zoom))
                        } else {
                            pxl_spec::procedural::node_seed(0xca7e_0001, (zoom, c))
                        };
                        let got = u64::from(words[2 * i]) | u64::from(words[2 * i + 1]) << 32;
                        if got != want {
                            seed_differ += 1;
                            if seed_differ <= 2 {
                                println!("  seed z{zoom} {c:?}: gpu {got:016x} cpu {want:016x}");
                            }
                        }
                    }
                }
            }
            println!("z{zoom} x from {x0}: seeds differ {seed_differ}");
            println!("z{zoom} x from {x0}: {n} nodes, {hollow} hollow; gpu vs pxl-spec {here}, probe(cpu) vs pxl-spec {probe_differ}, gpu vs probe(cpu) {probe_gpu}");
            asked += n;
            differ += here;
        }
    }
    println!("total: {asked} nodes, {differ} differ");
}
