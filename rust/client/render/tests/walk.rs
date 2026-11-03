//! The walks by hand (frame/shader/walk.wgsl) answer as `cast::walk` does: a world
//! made up here, the Rust walk over it recording what it asks, the observed pyramid
//! filled with those answers, the hand passes run on a device, and each ray compared.
//! Skipped where no adapter answers.

use std::collections::BTreeMap;

use pxl_engine::answer::{In, Kind, AIR, FILL_SHIFT};
use pxl_engine::beam::{self, Grid};
use pxl_engine::cast::{
    self, Detail, End, Keep, Node, Query, Ray, Root, Spacing, Trace, Until, DEPTH,
};
use pxl_engine::light::{self, Light, Starts};
use pxl_engine::{eye, pyramid, ray, Zoom, FRAC_W};

/// The hash a node's made-up content turns on.
fn mix(zoom: Zoom, c: [i32; 3]) -> u32 {
    let mut h = zoom.wrapping_mul(0x9E37_79B1) ^ 0x51ED_270B;
    for v in c {
        h = (h ^ v as u32).wrapping_mul(0x85EB_CA77).rotate_left(13);
    }
    h ^ h >> 15
}

/// Rolling ground: its top, in units, at the column at `[x, y]` units.
fn top(x: i32, y: i32) -> i32 {
    let at = |v: i32| v as f32 * 0.02;
    ((libm::sinf(at(x)) + libm::cosf(at(y))) * 6.0 + 16.0) as i32
}

/// A box's words, `first` and `past` its steps on each axis (`answer::box_word`).
fn box_words(first: [u32; 3], past: [u32; 3]) -> [u32; 2] {
    [
        first[0] | (past[0] - 1) << 10 | first[1] << 20,
        (past[1] - 1) | first[2] << 10 | (past[2] - 1) << 20,
    ]
}

/// The box a split of `zoom` at `c` says its content lies in: short of its node only
/// where one lies among the ground's nodes.
fn box_of(zoom: Zoom, c: [i32; 3]) -> ([u32; 3], [u32; 3]) {
    match if zoom < 16 { 3 } else { mix(zoom, c) % 4 } {
        0 => ([0, 0, 0], [1024, 1024, 512]),
        1 => ([64, 32, 0], [1024, 1000, 1024]),
        _ => ([0, 0, 0], [1024, 1024, 1024]),
    }
}

/// Whether a box over `c` does not hold it whole: then it holds nothing, as a box says.
fn outside_a_box(zoom: Zoom, c: [i32; 3]) -> bool {
    (16..zoom).any(|z| {
        let d = zoom - z;
        let over = c.map(|v| v >> d);
        let (first, past) = box_of(z, over);
        (0..3).any(|a| {
            let low = (c[a] - (over[a] << d)) as i64;
            // A cell not wholly in it: a box holds all it says fills.
            let (s0, s1) = ((low * 1024) >> d, ((low + 1) * 1024 + (1 << d) - 1) >> d);
            s0 < first[a] as i64 || s1 > past[a] as i64
        })
    })
}

/// The made-up world, every answer it gave kept for the pyramid.
/// A node and the zooms it was asked at.
type Asked = (Zoom, [i32; 3], [Zoom; 2]);

#[derive(Default)]
struct World {
    asked: BTreeMap<Asked, In<[u32; 2]>>,
}

impl World {
    fn answer(zoom: Zoom, c: [i32; 3], [_, near]: [Zoom; 2]) -> In<[u32; 2]> {
        let h = mix(zoom, c);
        // Nodes of the world's ground only; a few not held.
        if zoom >= 8 && h.is_multiple_of(211) {
            return In {
                kind: Kind::Lacks,
                asset: [0, 0],
            };
        }
        if outside_a_box(zoom, c) {
            return In {
                kind: Kind::Cell,
                asset: AIR,
            };
        }
        // Where it lies, in units: zooms past the unit's are its unit's.
        let shift = 24 - zoom.min(24);
        let at = |v: i32| {
            if zoom > 24 {
                v >> (zoom - 24)
            } else {
                v.wrapping_shl(shift)
            }
        };
        let (x, y, z) = (at(c[0]), at(c[1]), at(c[2]));
        let ground = top(x, y);
        if zoom >= near {
            let asset = if z <= ground {
                let k = if h.is_multiple_of(5) { (h >> 8) % 8 } else { 0 };
                (5 + (h >> 16) % 40) | k << FILL_SHIFT
            } else {
                AIR[0]
            };
            return In {
                kind: Kind::Cell,
                asset: [asset, h >> 3 & 0xffff],
            };
        }
        if z > ground + 24 && !h.is_multiple_of(7) {
            return In {
                kind: Kind::Cell,
                asset: AIR,
            };
        }
        let (first, past) = box_of(zoom, c);
        let asset = box_words(first, past);
        In {
            kind: Kind::Split,
            asset,
        }
    }
}

impl Query for World {
    type At = ();
    fn root(&mut self, _: [i32; 3]) {}
    fn child(&mut self, _: &(), _: Zoom, _: [i32; 3]) {}
    fn node(
        &mut self,
        _: &mut (),
        zoom: Zoom,
        c: [i32; 3],
        zooms: [Zoom; 2],
        _: f32,
    ) -> In<[u32; 2]> {
        let a = World::answer(zoom, c, zooms);
        self.asked.insert((zoom, c, zooms), a);
        a
    }
}

/// A beam ray's walk needs a line kept; the hand pass writes it.
struct Line;
impl Keep<()> for Line {
    fn keep(&mut self, _: &Trace<()>, _: &[(); DEPTH]) {}
}

/// The pyramid with every answer kept, as `Writing` keeps one; how many found no slot.
fn pyramid_of(world: &World) -> (Vec<u32>, u32) {
    let mut w = vec![0u32; pyramid::WORDS as usize];
    let mut dropped = 0;
    let tag = pyramid::tag(1, 0);
    for (&(zoom, c, zooms), a) in &world.asked {
        if !pyramid::keeps(zoom, c) {
            continue;
        }
        let first = pyramid::entry_of(zoom, c);
        let Some(slot) = (first..first + pyramid::BUCKET)
            .map(|s| pyramid::ENTRIES_AT + s * pyramid::ENTRY_WORDS)
            .find(|&e| w[e as usize] == 0)
        else {
            dropped += 1;
            continue;
        };
        let s = slot as usize;
        // Some air as a turned eye leaves it, held at a coarser near: gone past alike.
        let air = matches!(a.kind, Kind::Cell) && a.asset == AIR;
        let held = if air && mix(zoom, c).is_multiple_of(3) {
            [zooms[0], zooms[1].saturating_sub(1)]
        } else {
            zooms
        };
        // Some splits as a moved brick leaves them: forgotten, gone through alike.
        let key = pyramid::key(tag, held);
        let forgotten = matches!(a.kind, Kind::Split) && mix(zoom, c).is_multiple_of(9);
        w[s + pyramid::W_KEY as usize] = if forgotten {
            pyramid::forgotten_split(key)
        } else {
            key
        };
        w[s + pyramid::W_KIND as usize] = pyramid::stamped(a.kind, key, a.asset, [0; 4], zoom, c);
        w[s + pyramid::W_ASSET as usize] = a.asset[0];
        w[s + pyramid::W_ASSET as usize + 1] = a.asset[1];
        let n = s + pyramid::W_NODE as usize;
        w[n] = zoom;
        w[n + 1] = c[0] as u32;
        w[n + 2] = c[1] as u32;
        w[n + 3] = c[2] as u32;
    }
    (w, dropped)
}

const WORKGROUP: u32 = 8;

/// `(buffer, read_write)` a binding each, as `layout: None` finds them.
fn run(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    source: &str,
    entry: &str,
    bindings: &[(u32, wgpu::BindingResource)],
    groups: [u32; 2],
) {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(entry),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some(entry),
        layout: None,
        module: &module,
        entry_point: Some(entry),
        compilation_options: Default::default(),
        cache: None,
    });
    let entries: Vec<_> = bindings
        .iter()
        .map(|(b, resource)| wgpu::BindGroupEntry {
            binding: *b,
            resource: resource.clone(),
        })
        .collect();
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &entries,
    });
    let mut e = device.create_command_encoder(&Default::default());
    {
        let mut pass = e.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.dispatch_workgroups(groups[0], groups[1], 1);
    }
    queue.submit([e.finish()]);
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
}

fn read(device: &wgpu::Device, queue: &wgpu::Queue, from: &wgpu::Buffer) -> Vec<u32> {
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: from.size(),
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut e = device.create_command_encoder(&Default::default());
    e.copy_buffer_to_buffer(from, 0, &staging, 0, from.size());
    queue.submit([e.finish()]);
    staging
        .slice(..)
        .map_async(wgpu::MapMode::Read, |r| r.unwrap());
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    let v = staging.slice(..).get_mapped_range().unwrap();
    v.as_chunks::<4>()
        .0
        .iter()
        .map(|b| u32::from_le_bytes(*b))
        .collect()
}

#[test]
fn hand_walk_answers_as_cast_walk() {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let Ok(adapter) = pollster::block_on(instance.request_adapter(&Default::default())) else {
        eprintln!("no adapter: skipped");
        return;
    };
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits: adapter.limits(),
        ..Default::default()
    }))
    .unwrap();

    // The eye, over the ground, looking along it and down.
    let origin = [(3000 << 8) | 5, (2000 << 8) | 77, (45 << 8) | 13];
    let forward = {
        let d = [0.71f32, 0.52, -0.47];
        let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        [d[0] / l, d[1] / l, d[2] / l]
    };
    let right = {
        let l = (forward[0] * forward[0] + forward[1] * forward[1]).sqrt();
        [forward[1] / l, -forward[0] / l, 0.0]
    };
    let up = [
        right[1] * forward[2] - right[2] * forward[1],
        right[2] * forward[0] - right[0] * forward[2],
        right[0] * forward[1] - right[1] * forward[0],
    ];
    let far = 2000.0f32;
    let tan = [0.3f32, 0.2];
    let range = [4u32, 24];
    let grid = Grid::of(tan, 1);

    let mut eye_words = vec![0u32; eye::WORDS];
    for k in 0..3 {
        eye_words[eye::ORIGIN + k] = origin[k] as u32;
        eye_words[eye::FORWARD + k] = forward[k].to_bits();
        eye_words[eye::RIGHT + k] = right[k].to_bits();
        eye_words[eye::UP + k] = up[k].to_bits();
    }
    eye_words[eye::FAR] = far.to_bits();
    eye_words[eye::TAN] = tan[0].to_bits();
    eye_words[eye::TAN + 1] = tan[1].to_bits();
    eye_words[eye::RANGE] = range[0];
    eye_words[eye::RANGE + 1] = range[1];
    eye_words[eye::GRID..eye::GRID + eye::GRID_WORDS].copy_from_slice(&grid.words());

    let detail = Detail {
        range,
        spacing: Spacing {
            angle: grid.s,
            across: 0.0,
        },
    };
    let aim = |at: [f32; 2]| Ray::new(cast::through(forward, right, up, at[0], at[1]), far);

    // The Rust walks: every beam ray, then every sample from its cell's start.
    let mut world = World::default();
    let mut beams_cpu = Vec::new();
    for j in 0..grid.corners[1] {
        for i in 0..grid.corners[0] {
            let ray = aim(grid.corner([i, j]));
            let met =
                cast::walk_from_root(&mut world, origin, &ray, detail, Until::Beam, &mut Line);
            beams_cpu.push(met);
        }
    }
    let mut beams_gpu = vec![0u32; grid.size() as usize];
    for y in 0..grid.span[1] {
        for x in 0..grid.span[0] {
            // Each cell starts off its first corner, as far as that hands on.
            let c = grid.first([x, y]);
            let idx = c[1] * grid.corners[0] + c[0];
            let met = &beams_cpu[idx as usize];
            let at = grid.cell_at([x, y]) as usize;
            beams_gpu[at + beam::CELL_T as usize] = grid.hand_dist(met.hit.t).to_bits();
            beams_gpu[at + beam::CELL_CORNER as usize] = idx;
            beams_gpu[at + beam::CELL_RESUMES as usize] = 1;
        }
    }
    // Each corner's line's node, as a walk resumes off it.
    for (idx, met) in beams_cpu.iter().enumerate() {
        let k = (Grid::line(idx as u32) + beam::LINE_NODE) as usize;
        let n = &met.hit.node;
        beams_gpu[k + beam::NODE_ZOOM as usize] = n.zoom;
        for a in 0..3 {
            beams_gpu[k + beam::NODE_CORNER as usize + a] = n.corner[a];
            beams_gpu[k + beam::NODE_ROOTS as usize + a] = n.roots[a] as u32;
        }
    }
    let mut samples_cpu = Vec::new();
    for j in 0..grid.samples[1] {
        for i in 0..grid.samples[0] {
            let ray = aim(grid.sample([i, j]));
            let cell = grid.sample_cell([i, j]);
            let c = grid.first(cell);
            let met = &beams_cpu[(c[1] * grid.corners[0] + c[0]) as usize];
            let t = grid.hand_dist(met.hit.t);
            let (h, held) = met.hit.node.over(cast::point_at(origin, &ray, t));
            let from = if held {
                (cast::holding(origin, &ray, h, t), (), t)
            } else {
                (Node::ROOT, (), t)
            };
            samples_cpu.push(cast::walk(
                &mut world,
                origin,
                &ray,
                detail,
                Until::Opaque,
                from,
                &Root(()),
                &mut (),
            ));
        }
    }

    // The table as an eye a little away left it, its nodes asked at other zooms, each
    // kept beside what this eye asked: what a held split at other zooms lets the walk
    // go on through must end where this eye's own answers do.
    let mut earlier = World::default();
    let away = [
        origin[0] - (40 << 8),
        origin[1] + (25 << 8),
        origin[2] + (9 << 8),
    ];
    for j in (0..grid.samples[1]).step_by(3) {
        for i in (0..grid.samples[0]).step_by(3) {
            let ray = aim(grid.sample([i, j]));
            cast::first(&mut earlier, away, &ray, detail, Until::Opaque);
        }
    }
    let only_earlier: Vec<_> = earlier
        .asked
        .iter()
        .filter(|(k, a)| {
            matches!(a.kind, Kind::Split) && !world.asked.keys().any(|h| h.0 == k.0 && h.1 == k.1)
        })
        .map(|(k, a)| (*k, *a))
        .collect();
    let stale = only_earlier.len();
    world.asked.extend(only_earlier);
    let dropped_cells: Vec<_> = {
        // This eye's own splits dropped at random, so a walk meets them only at the
        // earlier eye's zooms where those were kept.
        let keys: Vec<_> = world
            .asked
            .keys()
            .filter(|k| {
                matches!(world.asked[*k].kind, Kind::Split)
                    && earlier
                        .asked
                        .keys()
                        .any(|h| h.0 == k.0 && h.1 == k.1 && h.2 != k.2)
            })
            .copied()
            .collect();
        keys
    };
    let reasked = dropped_cells.len();
    for k in &dropped_cells {
        let h = earlier
            .asked
            .keys()
            .find(|h| h.0 == k.0 && h.1 == k.1 && h.2 != k.2)
            .copied()
            .unwrap();
        let a = world.asked.remove(k).unwrap();
        world.asked.insert(h, a);
    }
    eprintln!("{stale} splits only the earlier eye asked, {reasked} held at its zooms alone");
    let (table, dropped) = pyramid_of(&world);
    use wgpu::util::DeviceExt as _;
    let buffer = |words: &[u32], usage: wgpu::BufferUsages| {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            // SAFETY: every u32's bytes are u8s, in the slice's own lifetime.
            contents: unsafe { std::slice::from_raw_parts(words.as_ptr().cast(), words.len() * 4) },
            usage: usage | wgpu::BufferUsages::COPY_SRC,
        })
    };
    let eye_buf = buffer(&eye_words, wgpu::BufferUsages::UNIFORM);
    let beams_buf = buffer(&beams_gpu, wgpu::BufferUsages::STORAGE);
    let pyramid_buf = buffer(&table, wgpu::BufferUsages::STORAGE);
    let left = buffer(&[0, 1, 1, 0], wgpu::BufferUsages::STORAGE);
    let source = |parts: [&str; 4]| parts.join("\n");
    let sample_src = source([
        include_str!(concat!(env!("OUT_DIR"), "/shader/walk_word.wgsl")),
        include_str!(concat!(env!("OUT_DIR"), "/shader/walk_node.wgsl")),
        include_str!(concat!(env!("OUT_DIR"), "/shader/walk.wgsl")),
        include_str!(concat!(env!("OUT_DIR"), "/shader/sample_hit.wgsl")),
    ]);
    let binds = [
        (0, eye_buf.as_entire_binding()),
        (8, beams_buf.as_entire_binding()),
        (9, pyramid_buf.as_entire_binding()),
        (10, left.as_entire_binding()),
    ];
    run(
        &device,
        &queue,
        &sample_src,
        "sample_hit_cs",
        &binds,
        [
            grid.samples[0].div_ceil(WORKGROUP),
            grid.samples[1].div_ceil(WORKGROUP),
        ],
    );
    let out = read(&device, &queue, &beams_buf);
    let left_words = read(&device, &queue, &left);

    // The beam rays', over the same table, from a line of their own.
    let beam_left = buffer(&[0, 1, 1, 0], wgpu::BufferUsages::STORAGE);
    let beam_buf = buffer(
        &vec![0u32; grid.size() as usize],
        wgpu::BufferUsages::STORAGE,
    );
    let beam_src = source([
        include_str!(concat!(env!("OUT_DIR"), "/shader/walk_word.wgsl")),
        include_str!(concat!(env!("OUT_DIR"), "/shader/walk_node.wgsl")),
        include_str!(concat!(env!("OUT_DIR"), "/shader/walk.wgsl")),
        include_str!(concat!(env!("OUT_DIR"), "/shader/beam_hit.wgsl")),
    ]);
    run(
        &device,
        &queue,
        &beam_src,
        "beam_hit_cs",
        &[
            (0, eye_buf.as_entire_binding()),
            (8, beam_buf.as_entire_binding()),
            (9, pyramid_buf.as_entire_binding()),
            (10, beam_left.as_entire_binding()),
        ],
        [
            grid.corners[0].div_ceil(WORKGROUP),
            grid.corners[1].div_ceil(WORKGROUP),
        ],
    );
    let lines = read(&device, &queue, &beam_buf);
    let beams_left = read(&device, &queue, &beam_left)[3];
    let mut was_left = vec![false; beams_cpu.len()];
    for p in 0..beams_left {
        let s = lines[(grid.corners_at + 1 + p) as usize];
        was_left[((s >> 16) * grid.corners[0] + (s & 0xffff)) as usize] = true;
    }
    let (mut beam_same, mut beam_differ, mut beam_first) = (0u32, 0u32, None);
    for (idx, met) in beams_cpu.iter().enumerate() {
        if was_left[idx] {
            continue;
        }
        let k = (Grid::line(idx as u32) + beam::LINE_NODE) as usize;
        let n = &met.hit.node;
        let hand = f32::from_bits(lines[grid.hand_at(idx as u32) as usize]);
        let want = grid.hand_dist(met.hit.t);
        let ok = lines[k + beam::NODE_ZOOM as usize] == n.zoom
            && (0..3).all(|a| {
                lines[k + beam::NODE_CORNER as usize + a] == n.corner[a]
                    && lines[k + beam::NODE_ROOTS as usize + a] == n.roots[a] as u32
            })
            // More where it went in a split whose box it took no word of.
            && lines[Grid::line(idx as u32) as usize + beam::LINE_VISITS as usize] >= met.visits
            && (hand - want).abs() <= want.abs() * 1e-3 + 1e-3;
        if ok {
            beam_same += 1;
        } else {
            beam_differ += 1;
            beam_first.get_or_insert((idx, met.ends, *n, met.visits, hand, want));
        }
    }
    eprintln!("beams: {beam_same} alike, {beam_differ} differ, {beams_left} left");
    assert!(beam_same > 500, "too few beams to say: {beam_same}");
    // Air held at other zooms asked again, one ray in `REVISIT_ONE_IN`: left, not gone past.
    assert!(beams_left > 800, "too few left: {beams_left}");
    assert_eq!(beam_differ, 0, "first: {beam_first:?}");

    let (mut same, mut differ, mut first) = (0u32, 0u32, None);
    let mut listed = vec![false; (grid.samples[0] * grid.samples[1]) as usize];
    for p in 0..left_words[3] {
        let s = out[(grid.lefts_at + 1 + p) as usize];
        listed[((s >> 16) * grid.samples[0] + (s & 0xffff)) as usize] = true;
    }
    for j in 0..grid.samples[1] {
        for i in 0..grid.samples[0] {
            let n = (j * grid.samples[0] + i) as usize;
            if listed[n] {
                continue;
            }
            let met = &samples_cpu[n];
            let b = grid.sample_at([i, j]) as usize;
            let node = b + beam::SAMPLE_NODE as usize;
            let want_zoom = if met.ends == End::Miss {
                beam::SAMPLE_NONE
            } else {
                met.hit.node.zoom
            };
            let ok = out[node + beam::NODE_ZOOM as usize] == want_zoom
                && (met.ends == End::Miss
                    || (0..3).all(|a| {
                        out[node + beam::NODE_CORNER as usize + a] == met.hit.node.corner[a]
                            && out[node + beam::NODE_ROOTS as usize + a]
                                == met.hit.node.roots[a] as u32
                    }) && out[b + beam::SAMPLE_ASSET as usize] == met.hit.asset[0]
                        && out[b + beam::SAMPLE_AXIS as usize] == met.hit.axis
                        && out[b + beam::SAMPLE_LACKS as usize] == (met.ends == End::Lacks) as u32);
            if ok {
                same += 1;
            } else {
                differ += 1;
                first.get_or_insert(([i, j], met.ends, met.hit.node, out[b..b + 12].to_vec()));
            }
        }
    }
    eprintln!(
        "samples: {same} alike, {differ} differ, {} left, {dropped} of {} answers found no slot",
        left_words[3],
        world.asked.len()
    );
    assert!(
        world.asked.len() > 1000,
        "too few nodes to say: {}",
        world.asked.len()
    );
    assert!(same > 1000, "the walk ended too few to say: {same}");
    assert!(left_words[3] > 3500, "too few left: {}", left_words[3]);
    let hits = samples_cpu.iter().filter(|m| m.ends == End::Hit).count();
    assert!(hits > 1000, "too few hits to say: {hits}");
    assert_eq!(differ, 0, "first: {first:?}");
}

/// The shadow map's missed slots: what `light_miss_hit_cs` writes, a slot's word
/// as a walk of `cast` leaves it (`light::depth`, `light::word`), at each ray's own band.
#[test]
fn hand_light_walk_answers_as_cast_walk() {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let Ok(adapter) = pollster::block_on(instance.request_adapter(&Default::default())) else {
        eprintln!("no adapter: skipped");
        return;
    };
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits: adapter.limits(),
        ..Default::default()
    }))
    .unwrap();

    const SIZE: u32 = 256;
    const WITHIN: u32 = 1024;
    let origin = [(3000 << 8) | 5, (2000 << 8) | 77, (45 << 8) | 13];
    let range = [4u32, 24];
    let unit = |v: [f32; 3]| {
        let l = libm::sqrtf(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
        [v[0] / l, v[1] / l, v[2] / l]
    };
    let cross = |a: [f32; 3], b: [f32; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let dir = unit([0.3, 0.2, -0.93]);
    let across = unit(cross(dir, [0.0, 0.0, 1.0])).map(|v| v * 3.0);
    let down = unit(cross(dir, unit(across))).map(|v| v * 3.0);
    let corner = [1300, -900];
    let half = SIZE as f32 / 2.0;
    let s = Light {
        first: [0, 1, 2].map(|a| -half * (across[a] + down[a]) - 120.0 * dir[a]),
        t_max: 400.0,
        across,
        down,
        dir,
        bias: [0.00025, 0.0005, 0.00075],
        corner,
        reach_ndc: 1.0,
    };
    // As a host lays them for the eye (light.rs `Starts`).
    let base = corner.map(|c| c.div_euclid(light::ANCHOR) * light::ANCHOR);
    let q3 = (1u32 << FRAC_W) as f32;
    let anchor = |k: usize| {
        let w = [
            base[0] + (k % 2) as i32 * light::ANCHOR,
            base[1] + (k / 2) as i32 * light::ANCHOR,
        ];
        let (i, j) = ((w[0] - corner[0]) as f32, (w[1] - corner[1]) as f32);
        let v = |a: usize| (s.first[a] + i * across[a] + j * down[a]) * q3;
        (
            [0, 1, 2].map(|a| origin[a] + libm::rintf(v(a)) as i32),
            [0, 1, 2].map(|a| v(a) - libm::rintf(v(a))),
        )
    };
    let [a0, a1, a2, a3] = [0, 1, 2, 3].map(anchor);
    let starts = Starts {
        base,
        across: across.map(|v| v * q3),
        down: down.map(|v| v * q3),
        at: [a0.0, a1.0, a2.0, a3.0],
        frac: [a0.1, a1.1, a2.1, a3.1],
    };

    let mut eye_words = vec![0u32; eye::WORDS];
    eye_words[eye::LIGHT..eye::LIGHT + eye::LIGHT_WORDS].copy_from_slice(&s.words());
    eye_words[eye::RANGE] = range[0];
    eye_words[eye::RANGE + 1] = range[1];
    let mut head = vec![0u32; light::HEAD_WORDS];
    head[light::HEAD_CAST] = 7;
    head[light::HEAD_SIZE] = SIZE;
    head[light::STARTS_AT * 4..(light::STARTS_AT + light::STARTS_VEC4S) * 4]
        .copy_from_slice(&starts.words());

    // Every slot's walk, its word; the slots, wrapped as the box's corner lays them.
    let mut world = World::default();
    let mut list = vec![SIZE * SIZE];
    let mut want = vec![0u32; (SIZE * SIZE) as usize];
    let mut stopped = 0;
    for y in 0..SIZE {
        for x in 0..SIZE {
            let slot = [x, y];
            let p = s.texel(SIZE, slot);
            let (o, r) = light::ray(&s, &starts, p);
            let tile = [(p[0] / light::TILE) as i32, (p[1] / light::TILE) as i32];
            let band = s.band(SIZE, range, tile);
            let detail = Detail {
                range: [band, band],
                spacing: s.spacing(),
            };
            let met = cast::first(&mut world, o, &r, detail, Until::Opaque);
            stopped += u32::from(met.ends == End::Hit);
            want[(y * SIZE + x) as usize] = light::word(light::depth(&s, &met));
            list.push(x | (y << 16));
        }
    }
    let (table, _) = pyramid_of(&world);

    use wgpu::util::DeviceExt as _;
    let buffer = |words: &[u32], usage: wgpu::BufferUsages| {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            // SAFETY: every u32's bytes are u8s, in the slice's own lifetime.
            contents: unsafe { std::slice::from_raw_parts(words.as_ptr().cast(), words.len() * 4) },
            usage: usage | wgpu::BufferUsages::COPY_SRC,
        })
    };
    let eye_buf = buffer(&eye_words, wgpu::BufferUsages::UNIFORM);
    let head_buf = buffer(&head, wgpu::BufferUsages::UNIFORM);
    let list_buf = buffer(&list, wgpu::BufferUsages::STORAGE);
    let pyramid_buf = buffer(&table, wgpu::BufferUsages::STORAGE);
    let left = buffer(&[ray::MISS_ACROSS, 0, 1, 0], wgpu::BufferUsages::STORAGE);
    let map = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R32Uint,
        usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = map.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    });
    let source = [
        include_str!(concat!(env!("OUT_DIR"), "/shader/walk_word.wgsl")),
        include_str!(concat!(env!("OUT_DIR"), "/shader/walk_node.wgsl")),
        include_str!(concat!(env!("OUT_DIR"), "/shader/walk.wgsl")),
        include_str!(concat!(env!("OUT_DIR"), "/shader/light_aim.wgsl")),
        include_str!(concat!(env!("OUT_DIR"), "/shader/light_miss_hit.wgsl")),
    ]
    .join("\n");
    run(
        &device,
        &queue,
        &source,
        "light_miss_hit_cs",
        &[
            (0, eye_buf.as_entire_binding()),
            (8, list_buf.as_entire_binding()),
            (9, pyramid_buf.as_entire_binding()),
            (10, wgpu::BindingResource::TextureView(&view)),
            (11, head_buf.as_entire_binding()),
            (13, left.as_entire_binding()),
        ],
        [
            ray::MISS_ACROSS,
            (SIZE * SIZE).div_ceil(ray::MISS_LANES * ray::MISS_ACROSS),
        ],
    );

    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(SIZE * SIZE * 4),
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut e = device.create_command_encoder(&Default::default());
    e.copy_texture_to_buffer(
        map.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &staging,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(SIZE * 4),
                rows_per_image: None,
            },
        },
        map.size(),
    );
    queue.submit([e.finish()]);
    staging
        .slice(..)
        .map_async(wgpu::MapMode::Read, |r| r.unwrap());
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    let got: Vec<u32> = staging
        .slice(..)
        .get_mapped_range()
        .unwrap()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| u32::from_le_bytes(*b))
        .collect();
    let after = read(&device, &queue, &list_buf);
    let (mut same, mut differ, mut slots_left, mut hits) = (0, 0, 0, 0);
    let mut steps_over = 0;
    let mut first = None;
    for k in 0..(SIZE * SIZE) as usize {
        if after[k + 1] != light::MISS_STRUCK {
            slots_left += 1;
            assert_eq!(got[k], 0, "a slot left wrote its word");
            continue;
        }
        // Far under the bias: a GPU rounds a division and a sum its own way, as
        // `light_miss_cs` does. A ray's start a Q3 step over moves a grazing hit most.
        if got[k].abs_diff(want[k]) <= WITHIN {
            same += 1;
            hits += u32::from(want[k] < light::DEPTH_ONE);
        } else {
            // A start a Q3 step over, which may clear a cube's edge and stop behind it.
            let p = s.texel(SIZE, [k as u32 % SIZE, k as u32 / SIZE]);
            let (o, r) = light::ray(&s, &starts, p);
            let tile = [(p[0] / light::TILE) as i32, (p[1] / light::TILE) as i32];
            let band = s.band(SIZE, range, tile);
            let detail = Detail {
                range: [band, band],
                spacing: s.spacing(),
            };
            let mut over = false;
            for n in 0..27 {
                let at = [o[0] + n % 3 - 1, o[1] + (n / 3) % 3 - 1, o[2] + n / 9 - 1];
                let met = cast::first(&mut world, at, &r, detail, Until::Opaque);
                over |= light::word(light::depth(&s, &met)).abs_diff(got[k]) <= WITHIN;
            }
            if over {
                steps_over += 1;
            } else {
                differ += 1;
                first.get_or_insert((k, got[k], want[k]));
            }
        }
    }
    eprintln!(
        "light: {same} alike ({hits} hits), {differ} differ, {slots_left} left, {stopped} stopped"
    );
    eprintln!("{steps_over} ended as a start a Q3 step over ends");
    assert!(steps_over < 32, "too many a step over: {steps_over}");
    assert!(slots_left < SIZE * SIZE / 4, "too many left: {slots_left}");
    assert!(slots_left > 13_000, "too few left: {slots_left}");
    assert!(same > 30_000, "too few slots to say: {same}");
    assert!(hits > 10_000, "too few hits to say: {hits}");
    // Air another band left held, gone past: at the band asked it may be what a ray
    // stops at (`slot_at_other_zooms`), until `revisits` asks again.
    assert!(
        differ * 1000 < same,
        "too many differ: {differ}, first: {first:?}"
    );
}
