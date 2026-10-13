//! Runs the WGSL rust-gpu made of pxl-spec's gpu-portable caves (cave.wgsl)
//! on the GPU over a tile's nodes, X across the wrap and every axis across
//! zero, and diffs each node's bits against the same source on the CPU,
//! pxl-spec's own `Cave` and `node_seed`.

use pxl_spec::procedural::{node_seed, Cave};
use wgpu::util::DeviceExt;

const ACROSS: [u32; 3] = [8, 8, 8];
const CAVES: u32 = 0xca7e_0001;

/// pxl-spec's answer, in the shader's bits: 0 hollow, 1 whole known, 2 whole's value.
fn cpu(zoom: u32, c: [i32; 3]) -> u32 {
    let cave = Cave::of(zoom, c);
    let (known, value) = cave.whole(zoom);
    u32::from(cave.hollow()) | u32::from(known) << 1 | u32::from(value) << 2
}

fn main() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).expect("adapter");
    let info = adapter.get_info();
    println!("adapter: {} ({:?}); no feature asked for", info.name, info.backend);
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).expect("device");
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../cave.wgsl")).expect("cave.wgsl");
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
    let (caves, seeds) = (pipeline_of("main_cs"), pipeline_of("seed_cs"));
    let n = (ACROSS[0] * ACROSS[1] * ACROSS[2]) as usize;
    // One entry over one region: its words.
    let run = |pipeline: &wgpu::ComputePipeline, ask: &wgpu::Buffer| -> Vec<u32> {
        let size = (n * 4) as u64;
        let out = device.create_buffer(&wgpu::BufferDescriptor { label: None, size, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC, mapped_at_creation: false });
        let read = device.create_buffer(&wgpu::BufferDescriptor { label: None, size, usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: ask.as_entire_binding() }, wgpu::BindGroupEntry { binding: 1, resource: out.as_entire_binding() }],
        });
        let mut enc = device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_compute_pass(&Default::default());
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &bind, &[]);
            pass.dispatch_workgroups(ACROSS[0] / 4, ACROSS[1] / 4, ACROSS[2] / 4);
        }
        enc.copy_buffer_to_buffer(&out, 0, &read, 0, size);
        queue.submit([enc.finish()]);
        read.slice(..).map_async(wgpu::MapMode::Read, |r| r.expect("map"));
        device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
        let words = read.slice(..).get_mapped_range().expect("mapped").chunks(4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
        words
    };
    let (mut asked, mut cave_differ, mut seed_differ, mut hollow) = (0usize, 0usize, 0usize, 0usize);
    for zoom in [19u32, 20, 22, 24] {
        let half = 1i32 << (zoom - 1);
        for x0 in [half - 4, -half - 4] {
            let first = [x0, -4, -4];
            let mut words = [0u32; 8];
            words[0] = zoom;
            for k in 0..3 {
                words[1 + k] = first[k] as u32;
                words[4 + k] = ACROSS[k];
            }
            let ask = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: &words.iter().flat_map(|w| w.to_le_bytes()).collect::<Vec<u8>>(),
                usage: wgpu::BufferUsages::STORAGE,
            });
            let (gpu_caves, gpu_seeds) = (run(&caves, &ask), run(&seeds, &ask));
            let (mut here, mut here_seed) = (0, 0);
            for z in 0..ACROSS[2] {
                for y in 0..ACROSS[1] {
                    for x in 0..ACROSS[0] {
                        let i = (x + ACROSS[0] * (y + ACROSS[1] * z)) as usize;
                        let c = [first[0] + x as i32, first[1] + y as i32, first[2] + z as i32];
                        let want = cpu(zoom, c);
                        hollow += (want & 1) as usize;
                        if gpu_caves[i] != want {
                            here += 1;
                            if here <= 3 {
                                println!("  cave z{zoom} {c:?}: gpu {:03b} cpu {want:03b}", gpu_caves[i]);
                            }
                        }
                        if gpu_seeds[i] != node_seed(CAVES, (zoom, c)) {
                            here_seed += 1;
                        }
                    }
                }
            }
            println!("z{zoom} x from {x0}: {n} nodes, caves differ {here}, seeds differ {here_seed}");
            asked += n;
            cave_differ += here;
            seed_differ += here_seed;
        }
    }
    println!("total: {asked} nodes, {hollow} hollow; caves differ {cave_differ}, seeds differ {seed_differ}");
}
