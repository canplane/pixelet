// gpu-query-runner/main.rs

//! Runs the harness's WGSL rust-gpu made of pxl-portable (`script/portable-wgsl.ts`
//! builds it beside the cast's, into gpu-query's target) on the GPU and
//! diffs each answer's bits against the same source on the CPU: the caves
//! and a node's seed over a tile's nodes, X across the wrap and every axis
//! across zero, as pxl-spec has them; and a leaf tile's ground, each cell's
//! look, height and block at a few heights, over a made-up tile.
//!
//! wgpu reads the WGSL through Naga, whose Metal backend (naga 30.0.1) ends
//! a `loop { … continuing { break if !c } }` an iteration early: it writes
//! `c`, a `let` of the loop's body, as its expression again after the
//! continuing block moved the counter. The WGSL means the `let`'s value. So
//! the surface's 4 × 4 sum differs here where its WGSL is right; a browser's
//! WebGPU compiles WGSL with its own compiler, not Naga.

use pxl_portable::ground::{self, Tile, ELE_PADDED_SPAN, TILE_CELLS};
use pxl_portable::LEAF_CELL;
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

/// A made-up leaf tile: looks over the palette, the sentinels among them,
/// and heights rolling over hills and under the sea, rough where they meet.
struct Made {
    looks: Vec<u8>,
    ele: Vec<i16>,
}

impl Tile for Made {
    fn look(&self, x: u32, y: u32) -> u32 {
        u32::from(self.looks[(y * TILE_CELLS + x) as usize])
    }
    fn ele(&self, i: usize) -> i32 {
        i32::from(self.ele[i])
    }
}

fn made() -> Made {
    let mut state = 0x9e37_79b9_7f4a_7c15u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let looks = (0..TILE_CELLS * TILE_CELLS)
        .map(|_| match next() % 16 {
            0 => 0x00,
            1 => 0xff,
            r => (r * 4 + next() % 4) as u8 % 64,
        })
        .collect();
    let side = ELE_PADDED_SPAN as usize;
    let ele = (0..side * side)
        .map(|i| {
            let (x, y) = ((i % side) as f64, (i / side) as f64);
            (300.0 * (x / 5.0).sin() * (y / 7.0).cos() + 40.0 * (x + y) / side as f64 + (next() % 9) as f64 - 4.0) as i16
        })
        .collect();
    Made { looks, ele }
}

/// pxl-portable's answer for a leaf cell, in `ground_cs`'s bits.
fn cpu_ground(tile: &Made, z: i32, [x, y]: [u32; 2]) -> u32 {
    let (look, h) = ground::read(tile, [x as i32, y as i32], 1);
    let cell = ground::Cell {
        look,
        top: ground::blocks(LEAF_CELL, h as f32),
        fold: [look; 4],
    };
    let (block, _) = cell.block(LEAF_CELL, z, false);
    look | block << 8 | (h as u32 & 0xffff) << 16
}

fn main() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).expect("adapter");
    let info = adapter.get_info();
    println!("adapter: {} ({:?}); no feature asked for", info.name, info.backend);
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).expect("device");
    let source = include_str!("../../target-linux/spirv-builder-harness/spirv-unknown-naga-wgsl/release/deps/gpu_query_shader.wgsl");
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("portable"),
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
    let (caves, seeds, grounds) = (pipeline_of("harness_main_cs"), pipeline_of("harness_seed_cs"), pipeline_of("harness_ground_cs"));
    let n = (ACROSS[0] * ACROSS[1] * ACROSS[2]) as usize;
    let words_of = |v: &[u32]| {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: &v.iter().flat_map(|w| w.to_le_bytes()).collect::<Vec<u8>>(),
            usage: wgpu::BufferUsages::STORAGE,
        })
    };
    // One entry over `n` answers, `groups` workgroups, bound past `ask` and the answers to `more`: its words.
    let run_over = |pipeline: &wgpu::ComputePipeline, ask: &wgpu::Buffer, n: usize, groups: [u32; 3], more: &[&wgpu::Buffer]| -> Vec<u32> {
        let size = (n * 4) as u64;
        let out = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let read = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[[ask, &out].as_slice(), more]
                .concat()
                .iter()
                .enumerate()
                .map(|(i, b)| wgpu::BindGroupEntry {
                    binding: i as u32,
                    resource: b.as_entire_binding(),
                })
                .collect::<Vec<_>>(),
        });
        let mut enc = device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_compute_pass(&Default::default());
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &bind, &[]);
            pass.dispatch_workgroups(groups[0], groups[1], groups[2]);
        }
        enc.copy_buffer_to_buffer(&out, 0, &read, 0, size);
        queue.submit([enc.finish()]);
        read.slice(..).map_async(wgpu::MapMode::Read, |r| r.expect("map"));
        device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
        let words = read
            .slice(..)
            .get_mapped_range()
            .expect("mapped")
            .chunks(4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect();
        words
    };
    let run = |pipeline: &wgpu::ComputePipeline, ask: &wgpu::Buffer| run_over(pipeline, ask, n, ACROSS.map(|a| a / 4), &[]);
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

    // The ground: 4 looks a word and 2 heights a word, low first.
    let tile = made();
    let looks: Vec<u32> = tile.looks.chunks(4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
    let mut halves = tile.ele.clone();
    halves.resize(halves.len().next_multiple_of(2), 0);
    let heights: Vec<u32> = halves.chunks(2).map(|h| u32::from(h[0] as u16) | u32::from(h[1] as u16) << 16).collect();
    let (looks, heights) = (words_of(&looks), words_of(&heights));
    let cells = (TILE_CELLS * TILE_CELLS) as usize;
    let (mut field_differ, mut aired) = ([0usize; 3], 0usize);
    for z in [-400, -70, -66, -10, -4, -1, 0, 3, 40, 150, 320] {
        let ask = words_of(&[LEAF_CELL, 0, 0, z as u32, TILE_CELLS, TILE_CELLS, 1, 0]);
        let gpu = run_over(&grounds, &ask, cells, [TILE_CELLS / 8, TILE_CELLS / 8, 1], &[&looks, &heights]);
        let mut shown = 0;
        for y in 0..TILE_CELLS {
            for x in 0..TILE_CELLS {
                let (g, c) = (gpu[(y * TILE_CELLS + x) as usize], cpu_ground(&tile, z, [x, y]));
                aired += usize::from(c >> 8 & 0xff == 0);
                for (f, mask) in [0xff, 0xff00, 0xffff_0000].into_iter().enumerate() {
                    field_differ[f] += usize::from(g & mask != c & mask);
                }
                if g != c && shown < 3 {
                    shown += 1;
                    println!("  ground z={z} [{x}, {y}]: gpu {g:08x} cpu {c:08x}");
                }
            }
        }
    }
    println!(
        "ground: {} cells × 11 heights, {aired} air; looks differ {}, blocks differ {}, heights differ {}",
        cells, field_differ[0], field_differ[1], field_differ[2]
    );
}
