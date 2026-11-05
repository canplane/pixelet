//! `decor_gate_cs` by hand (frame/shader/decor_gate.wgsl) answers as the rule it states: of each
//! region's instances, a frame through the gate lists exactly those whose ball (decor_ball.wgsl)
//! meets the claimed tiles' box as the frame sees it, at the region's own place, and the
//! region's args with their count. The instances lie about the view, some well inside the box,
//! some well past it; one within a hair of a plane is not asked either way, as a GPU's f32
//! rounds there.

mod gpu;

use std::collections::BTreeSet;

use gpu::Gpu;
use pxl_content::generation::{self, drawn};
use pxl_engine::screen;
use pxl_render::frame::shader::record::{
    compute, src, COMPUTE_DECOR_GATE, COMPUTE_DECOR_GATE_ARGS,
};
use pxl_render::frame::shader::word::FRAME_FLOATS;

const SIZE: [u32; 2] = [1080, 1920];
const SIDE: u32 = 32;
const CAP: u32 = 4096;
/// The claimed tiles' box, first and last across and down.
const BOX: [u32; 4] = [10, 20, 15, 31];

fn device() -> Option<Gpu> {
    let gpu = Gpu::new();
    if gpu.is_none() {
        eprintln!("no adapter: decor_gate skipped");
    }
    gpu
}

type M4 = [[f64; 4]; 4];

fn mul(a: &M4, b: &M4) -> M4 {
    let mut m = [[0.0; 4]; 4];
    for (r, row) in m.iter_mut().enumerate() {
        for (c, v) in row.iter_mut().enumerate() {
            *v = (0..4).map(|k| a[r][k] * b[k][c]).sum();
        }
    }
    m
}

/// A view from `eye` to the origin, Z up, and a perspective of `fov_y` radians over the
/// screen's aspect, rows as a matrix's.
fn view_proj(eye: [f64; 3], fov_y: f64) -> M4 {
    let norm = |v: [f64; 3]| {
        let l = libm::sqrt(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
        v.map(|x| x / l)
    };
    let cross = |a: [f64; 3], b: [f64; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let f = norm(eye.map(|v| -v));
    let s = norm(cross(f, [0.0, 0.0, 1.0]));
    let u = cross(s, f);
    let view = [
        [s[0], s[1], s[2], -dot(s, eye)],
        [u[0], u[1], u[2], -dot(u, eye)],
        [-f[0], -f[1], -f[2], dot(f, eye)],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let t = 1.0 / libm::tan(fov_y / 2.0);
    let aspect = f64::from(SIZE[0]) / f64::from(SIZE[1]);
    let (near, far) = (0.5, 5000.0);
    let proj = [
        [t / aspect, 0.0, 0.0, 0.0],
        [0.0, t, 0.0, 0.0],
        [0.0, 0.0, far / (near - far), near * far / (near - far)],
        [0.0, 0.0, -1.0, 0.0],
    ];
    mul(&proj, &view)
}

/// How far the ball at `c` of radius `r` lies inside the claimed box's four planes, the least
/// of them, in the planes' own units: past 0 inside, under it past.
fn inside(m: &M4, c: [f64; 3], r: f64) -> f64 {
    let side = f64::from(SIDE);
    let size = SIZE.map(f64::from);
    let lo = [
        f64::from(BOX[0]) * side - 1.0,
        f64::from(BOX[1]) * side - 1.0,
    ];
    let hi = [
        (f64::from(BOX[2] + 1) * side).min(size[0]) + 1.0,
        (f64::from(BOX[3] + 1) * side).min(size[1]) + 1.0,
    ];
    let x = [lo[0] / size[0] * 2.0 - 1.0, hi[0] / size[0] * 2.0 - 1.0];
    let y = [1.0 - lo[1] / size[1] * 2.0, 1.0 - hi[1] / size[1] * 2.0];
    let plane = |a: [f64; 4], k: f64, b: [f64; 4], sign: f64| -> [f64; 4] {
        std::array::from_fn(|i| sign * (a[i] - k * b[i]))
    };
    let planes = [
        plane(m[0], x[0], m[3], 1.0),
        plane(m[0], x[1], m[3], -1.0),
        plane(m[1], y[0], m[3], -1.0),
        plane(m[1], y[1], m[3], 1.0),
    ];
    planes
        .iter()
        .map(|p| {
            let n = libm::sqrt(p[0] * p[0] + p[1] * p[1] + p[2] * p[2]);
            (p[0] * c[0] + p[1] * c[1] + p[2] * c[2] + p[3] + r * n) / n
        })
        .fold(f64::INFINITY, f64::min)
}

#[test]
fn a_gated_frame_lists_the_instances_that_meet_its_box() {
    let Some(gpu) = device() else { return };
    let m = view_proj([0.0, -120.0, 60.0], 1.0);
    let translation = [3.0f32, -2.0, 0.5];
    // The regions' counts, some empty, laid end to end; each slot its own instance, listed in
    // reverse within the list.
    let regions = drawn::REGIONS as usize;
    let counts: Vec<u32> = (0..regions as u32)
        .map(|r| match r % 5 {
            0 => 0,
            1 => 37,
            2 => 120,
            3 => 9,
            _ => 64,
        })
        .collect();
    let total: u32 = counts.iter().sum();
    assert!(total <= CAP, "the regions hold {total}, past the slots");
    let columns = drawn::COLUMNS;
    let mut instances = vec![0u32; ((columns + 1) * CAP) as usize];
    let mut noise = 0x9e37_79b9u32;
    let mut next = || {
        noise ^= noise << 13;
        noise ^= noise >> 17;
        noise ^= noise << 5;
        f64::from(noise) / f64::from(u32::MAX)
    };
    for k in 0..total {
        let pos = [
            (next() - 0.5) * 300.0,
            next() * 400.0 - 60.0,
            (next() - 0.5) * 40.0,
        ];
        for (a, v) in pos.iter().enumerate() {
            instances[(a as u32 * CAP + k) as usize] = (*v as f32).to_bits();
        }
        instances[(3 * CAP + k) as usize] = 0.0f32.to_bits();
        instances[(4 * CAP + k) as usize] = 1.0f32.to_bits();
    }
    let list = (columns * CAP) as usize;
    let mut region_words = vec![0u32; 2 * regions];
    let mut uniforms = vec![0u32; drawn::UNIFORM_WORDS as usize * regions];
    let mut args = vec![0u32; drawn::ARGS_WORDS as usize * regions];
    let mut first = 0u32;
    for r in 0..regions {
        let n = counts[r];
        for k in 0..n {
            instances[list + (first + k) as usize] = first + n - 1 - k;
        }
        region_words[r] = n;
        region_words[regions + r] = first + n;
        let u = r * drawn::UNIFORM_WORDS as usize;
        uniforms[u..u + 3].copy_from_slice(&translation.map(f32::to_bits));
        uniforms[u + 4] = CAP;
        uniforms[u + drawn::UNIFORM_IDX as usize] = columns * CAP + first;
        let a = r * drawn::ARGS_WORDS as usize;
        args[a..a + 5].copy_from_slice(&[36, n, 7, 3, 0]);
        first += n;
    }
    let mut frame = vec![0u32; FRAME_FLOATS];
    for c in 0..4 {
        for r in 0..4 {
            frame[c * 4 + r] = (m[r][c] as f32).to_bits();
        }
    }
    let across = SIZE.map(|v| v.div_ceil(SIDE));
    let mut tiles = [0u32; screen::TILES_WORDS];
    tiles[..4].copy_from_slice(&[across[0], across[1], 1, 0]);
    tiles[4..]
        .copy_from_slice(&[SIZE[0] as f32, SIZE[1] as f32, 0.0, SIDE as f32].map(f32::to_bits));

    let storage = wgpu::BufferUsages::STORAGE;
    let frame_buf = gpu.buffer(&frame, wgpu::BufferUsages::UNIFORM);
    let uniforms_buf = gpu.buffer(&uniforms, storage);
    let args_buf = gpu.buffer(&args, storage);
    let instances_buf = gpu.buffer(&instances, storage);
    let gated = gpu.buffer(&vec![u32::MAX; instances.len()], storage);
    let gated_args = gpu.buffer(&vec![u32::MAX; args.len()], storage);
    let box_buf = gpu.buffer(&BOX, storage);
    let regions_buf = gpu.buffer(&region_words, storage);
    let tiles_buf = gpu.buffer(&tiles, wgpu::BufferUsages::UNIFORM);
    let binds = [
        (0, frame_buf.as_entire_binding()),
        (1, uniforms_buf.as_entire_binding()),
        (3, instances_buf.as_entire_binding()),
        (4, gated.as_entire_binding()),
        (5, gated_args.as_entire_binding()),
        (6, box_buf.as_entire_binding()),
        (7, regions_buf.as_entire_binding()),
        (31, tiles_buf.as_entire_binding()),
    ];
    let pipeline = |id| {
        let (shader, entry) = compute(id);
        gpu.pipeline(src(shader), entry)
    };
    gpu.dispatch(
        &pipeline(COMPUTE_DECOR_GATE_ARGS),
        &[
            (2, args_buf.as_entire_binding()),
            (5, gated_args.as_entire_binding()),
        ],
        [drawn::REGIONS.div_ceil(64), 1, 1],
    );
    gpu.dispatch(
        &pipeline(COMPUTE_DECOR_GATE),
        &binds,
        [CAP.div_ceil(64), 1, 1],
    );
    let got_args = gpu.read(&gated_args);
    let got = gpu.read(&gated);

    let radius = f64::from(generation::TALLEST + generation::WIDEST);
    let (mut listed, mut left, mut near, mut wrong) = (0, 0, 0, Vec::new());
    let mut first = 0u32;
    for (r, &count) in counts.iter().enumerate() {
        let a = r * drawn::ARGS_WORDS as usize;
        for w in [0, 2, 3, 4] {
            if got_args[a + w] != args[a + w] {
                wrong.push(format!(
                    "region {r}: args word {w} {} for {}",
                    got_args[a + w],
                    args[a + w]
                ));
            }
        }
        let n = got_args[a + 1];
        let at = list + first as usize;
        let have: BTreeSet<u32> = got[at..at + n as usize].iter().copied().collect();
        if have.len() != n as usize {
            wrong.push(format!(
                "region {r}: {n} listed, {} of them apart",
                have.len()
            ));
        }
        for k in 0..count {
            let slot = first + k;
            let v: [f64; 3] = std::array::from_fn(|a| {
                f64::from(f32::from_bits(instances[(a as u32 * CAP + slot) as usize]))
                    + f64::from(translation[a])
            });
            let middle = [v[0], v[1], v[2] + f64::from(generation::TALLEST) * 0.5];
            let d = inside(&m, middle, radius);
            if d.abs() < 1e-3 {
                near += 1;
                continue;
            }
            let wants = d > 0.0;
            if wants != have.contains(&slot) {
                wrong.push(format!(
                    "region {r} slot {slot}: listed {}, {d:+} from the box",
                    !wants
                ));
            }
            listed += u32::from(wants);
            left += u32::from(!wants);
        }
        first += count;
    }
    eprintln!("{total} instances: {listed} meet the box, {left} past it, {near} within a hair");
    assert!(
        listed > 0 && left > 0,
        "the case lists all or none: {listed} {left}"
    );
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
