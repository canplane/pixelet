//! What decides where the walks by hand (frame/shader/walk_node.wgsl) stop, against
//! pxl-engine: the zoom asked at a distance, whether a split is narrow at a beam's width, on
//! their edges, and which nodes a slot of the node cache keeps. Skipped where no adapter
//! answers.

mod gpu;

use pxl_engine::cast::{self, Spacing};
use pxl_engine::{grid, node_cache};
use pxl_render::frame::shader::record::{src, SHADER_QUEUE_CORNERS};

/// The probe's words a case: its vector's three, then what the module leaves of it.
const PROBE_WORDS: usize = 7;

/// The zoom and the narrowness as the walks work them out (walk_node.wgsl), a thread a case:
/// the vector's length squared and the zoom asked there, held to no range; and whether a split
/// `x` wide is narrow where a beam's rays lie `y` apart in angle, at `z` squared from the
/// origin.
const PROBE: &str = r"
@group(0) @binding(40) var<storage, read_write> probe: array<u32>;

@compute @workgroup_size(64, 1, 1)
fn probe_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x >= probe[0]) {
    return;
  }
  let k = 1u + id.x * 7u;
  let v = vec3<f32>(bitcast<f32>(probe[k]), bitcast<f32>(probe[k + 1u]), bitcast<f32>(probe[k + 2u]));
  let r2 = squared(v);
  probe[k + 3u] = bitcast<u32>(r2);
  probe[k + 4u] = zoom_at(r2, vec2<u32>(0u, FINEST_ZOOM));
  let width = spacing_width(vec2<f32>(0.0, v.y), v.z);
  probe[k + 5u] = bitcast<u32>(width);
  probe[k + 6u] = select(0u, 1u, narrow(v.x, width));
}
";

/// The probe run over `cases` on the beam rays' walk's module, what it left of each.
fn probe(gpu: &gpu::Gpu, cases: &[[f32; 3]]) -> Vec<[u32; PROBE_WORDS]> {
    let source = format!("{}\n{PROBE}", src(SHADER_QUEUE_CORNERS));
    let p = gpu.pipeline(&source, "probe_cs");
    let mut words = vec![cases.len() as u32];
    for v in cases {
        words.extend(v.map(f32::to_bits));
        words.extend([0; PROBE_WORDS - 3]);
    }
    let buf = gpu.buffer(&words, wgpu::BufferUsages::STORAGE);
    gpu.dispatch(
        &p,
        &[(40, buf.as_entire_binding())],
        [(cases.len() as u32).div_ceil(64), 1, 1],
    );
    let out = gpu.read(&buf);
    out[1..].as_chunks::<PROBE_WORDS>().0.to_vec()
}

/// pxl-engine cast.rs `squared`, whose order the walks sum in.
fn squared(v: [f32; 3]) -> f32 {
    v[0] * v[0] + v[1] * v[1] + v[2] * v[2]
}

fn next_up(x: f32) -> f32 {
    f32::from_bits(x.to_bits() + 1)
}

fn next_down(x: f32) -> f32 {
    f32::from_bits(x.to_bits() - 1)
}

/// How many last places a device's distance squared may lie off the Rust's: a product fused
/// into the sum (`squared`) is rounded once.
const FUSED_ULPS: u32 = 1;

/// The zoom the walks ask on a split distance, `2304 · 4^k` squared, and on the floats either
/// side of it is `grid::cell_zoom_at`'s, off a vector along an axis, one a little off it and
/// ones across all three axes. A device's distance squared lies `FUSED_ULPS` off the Rust's at
/// most, and its zoom is then the one at its own distance: each that took that edge counted.
#[test]
fn hand_zoom_on_a_split_distance() {
    let Some(gpu) = gpu::Gpu::new() else {
        eprintln!("no adapter: skipped");
        return;
    };
    // Each case's split distance squared, and the vectors whose squares sum by the Rust to it
    // or to the float either side.
    let mut cases = Vec::new();
    let mut seed = 0x2545_f491u32;
    let mut rand = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        f64::from(seed) / f64::from(u32::MAX)
    };
    for k in -7i32..=23 {
        let d = 48.0 * pow2(k);
        let r2 = d * d;
        assert_eq!(
            r2 * grid::INV_SPLIT2,
            pow2(2 * k),
            "a split distance scales to its power of four"
        );
        let mut put = |v: [f32; 3]| {
            let got = squared(v);
            if got == next_down(r2) || got == r2 || got == next_up(r2) {
                cases.push(v);
            }
        };
        put([d, 0.0, 0.0]);
        put([0.0, d, 0.0]);
        put([0.0, 0.0, d]);
        put([-d, 0.0, 0.0]);
        // A square an ulp of the distance's: the float over it.
        put([d, pow2(k - 6), 0.0]);
        put([0.0, -pow2(k - 6), d]);
        // The float under it: the next f32 under the distance squares to it.
        put([next_down(d), 0.0, 0.0]);
        put([0.0, 0.0, next_down(d)]);
        // Across all three axes: each run of floats about the one that sums to it.
        for _ in 0..24 {
            let (y, z) = (rand() * 0.6, rand() * 0.6);
            let x = (1.0 - y * y - z * z).sqrt();
            let v = [x, y, z].map(|c| (c * f64::from(d)) as f32);
            for n in -6i32..=6 {
                let x = f32::from_bits((v[0].to_bits() as i32 + n) as u32);
                put([x, v[1], -v[2]]);
            }
        }
    }
    let have = probe(&gpu, &cases);
    let (mut on, mut fused, mut differ) = ([0u32; 3], 0u32, Vec::new());
    for (v, h) in cases.iter().zip(&have) {
        let want_r2 = squared(*v);
        let want = grid::cell_zoom_at(want_r2);
        let (got_r2, got) = (f32::from_bits(h[3]), h[4]);
        // A zoom's edge within a last place of the Rust's distance squared.
        let edge = grid::cell_zoom_at(next_up(want_r2)) != want
            || grid::cell_zoom_at(next_down(want_r2)) != want;
        on[usize::from(edge)] += 1;
        if got == want && got_r2 == want_r2 {
            continue;
        }
        let off = (got_r2.to_bits() as i64 - want_r2.to_bits() as i64).unsigned_abs();
        if off <= u64::from(FUSED_ULPS) && got == grid::cell_zoom_at(got_r2) {
            fused += u32::from(got != want);
            continue;
        }
        differ.push((*v, want_r2, want, got_r2, got));
    }
    eprintln!(
        "{} cases, {} by an edge, {fused} took the fused edge's zoom, {} differ",
        cases.len(),
        on[1],
        differ.len()
    );
    assert!(on[1] > 200, "too few by an edge: {}", on[1]);
    assert!(differ.is_empty(), "first: {:?}", differ.first());
}

/// A split exactly `NARROW` beam widths wide, `side² = 8 · width²`, is narrow as `cast::narrow`
/// says, at powers of two of it, the width a spacing at a distance squared
/// (`Spacing::width`), and at the floats either side of the side.
#[test]
fn hand_narrow_on_its_edge() {
    let Some(gpu) = gpu::Gpu::new() else {
        eprintln!("no adapter: skipped");
        return;
    };
    // `1 / (2√2)`, whose square, rounded, is eight times one's, rounded.
    let w = f32::from_bits(0x3eb5_04f4);
    assert_eq!(
        8.0 * (w * w),
        1.000_000_1,
        "the side squares onto the bound"
    );
    let mut cases = Vec::new();
    for j in -10i32..=10 {
        let side = pow2(j);
        for m in [-4i32, 0, 3] {
            // An angle and a distance squared whose width is `w · side`, exactly.
            let (angle, d2) = (w * pow2(j - m), pow2(2 * m));
            for s in [next_down(side), side, next_up(side)] {
                cases.push([s, angle, d2]);
            }
        }
    }
    let have = probe(&gpu, &cases);
    let (mut narrow, mut apart_before, mut differ) = (0u32, 0u32, Vec::new());
    for (v, h) in cases.iter().zip(&have) {
        let width = Spacing {
            angle: v[1],
            across: 0.0,
        }
        .width(libm::sqrtf(v[2]));
        let want = cast::narrow(v[0] * v[0], width * width);
        narrow += u32::from(want);
        // The bound as `NARROW` squared in f32, a place under 8.
        apart_before +=
            u32::from((v[0] * v[0] < cast::NARROW * cast::NARROW * width * width) != want);
        if h[5] != width.to_bits() || (h[6] == 1) != want {
            differ.push((*v, width, want, f32::from_bits(h[5]), h[6]));
        }
    }
    eprintln!(
        "{} cases, {narrow} narrow, {apart_before} the bound `NARROW · NARROW` would part",
        cases.len()
    );
    assert!(apart_before > 0, "no case on the edge");
    assert!(differ.is_empty(), "first: {:?}", differ.first());
}

/// How many last places a device's beam width may lie off the Rust's, a rule a GPU alone
/// decides (walk_node.wgsl `spacing_width`): WGSL's square root is `1 / inverseSqrt`'s,
/// 2.5 places and 2, and the product by the angle rounds once more.
const WIDTH_ULPS: u32 = 5;

/// The width at a distance squared whose root no float holds exactly is `Spacing::width`'s
/// within `WIDTH_ULPS`, and a split on `NARROW` of the Rust's width, or a few floats either
/// side of it, is narrow as `cast::narrow` says of the device's own width: each width off
/// the Rust's counted, and each split whose narrowness that parted from the Rust's.
#[test]
fn hand_width_off_a_square_root() {
    let Some(gpu) = gpu::Gpu::new() else {
        eprintln!("no adapter: skipped");
        return;
    };
    let mut seed = 0x9e37_79b9u32;
    let mut rand = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        f64::from(seed) / f64::from(u32::MAX)
    };
    let width_of = |angle: f32, d2: f32| Spacing { angle, across: 0.0 }.width(libm::sqrtf(d2));
    let mut cases = Vec::new();
    for _ in 0..4096 {
        // A distance 2^-4 to 2^24 units, and the eye's lattice angle or finer.
        let d2 = libm::exp2(-8.0 + rand() * 56.0) as f32;
        let angle = libm::exp2(-6.0 - rand() * 8.0) as f32;
        let side = cast::NARROW * width_of(angle, d2);
        for n in -3i32..=3 {
            cases.push([
                f32::from_bits((side.to_bits() as i32 + n) as u32),
                angle,
                d2,
            ]);
        }
    }
    let have = probe(&gpu, &cases);
    let (mut off, mut most, mut parted, mut differ) = (0u32, 0u64, 0u32, Vec::new());
    for (v, h) in cases.iter().zip(&have) {
        let want = width_of(v[1], v[2]);
        let got = f32::from_bits(h[5]);
        let ulps = (i64::from(got.to_bits()) - i64::from(want.to_bits())).unsigned_abs();
        let narrow = h[6] == 1;
        off += u32::from(ulps != 0);
        most = most.max(ulps);
        parted += u32::from(narrow != cast::narrow(v[0] * v[0], want * want));
        if ulps > u64::from(WIDTH_ULPS) || narrow != cast::narrow(v[0] * v[0], got * got) {
            differ.push((*v, want, got, narrow));
        }
    }
    eprintln!(
        "{} cases, {off} widths off the Rust's, at most {most} places; {parted} splits parted by it",
        cases.len()
    );
    assert!(differ.is_empty(), "first: {:?}", differ.first());
}

/// Two to the `k`, exact: the float whose exponent is `k`, for `k` a normal float's.
fn pow2(k: i32) -> f32 {
    f32::from_bits(((127 + k) as u32) << 23)
}

/// Whether a slot keeps a node as the walks ask it and the bucket it is kept in
/// (walk_node.wgsl `keeps`, `bucket_of`), a thread a case: its zoom and its index, then what
/// the module leaves.
const KEEPS: &str = r"
@group(0) @binding(40) var<storage, read_write> probe: array<u32>;

@compute @workgroup_size(64, 1, 1)
fn keeps_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x >= probe[0]) {
    return;
  }
  let k = 1u + id.x * 6u;
  let c = vec3<i32>(i32(probe[k + 1u]), i32(probe[k + 2u]), i32(probe[k + 3u]));
  probe[k + 4u] = select(0u, 1u, keeps(probe[k], c));
  probe[k + 5u] = bucket_of(probe[k], c);
}
";

/// The walks keep a node exactly where pxl-engine's `node_cache::keeps` does, in the bucket its
/// `node_cache::bucket_of` names: at every zoom to past the finest, on every axis's index about
/// and across the edges of twice the world's span at that zoom, and on noise.
#[test]
fn hand_keeps_as_the_engine() {
    let Some(gpu) = gpu::Gpu::new() else {
        eprintln!("no adapter: skipped");
        return;
    };
    let mut seed = 0x9e37_79b9u32;
    let mut rand = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed
    };
    let mut cases: Vec<(u32, [i32; 3])> = Vec::new();
    for zoom in 0..=pxl_engine::FINEST_ZOOM + 2 {
        // The first and last index kept on an axis, and the one past each.
        let side = if zoom >= 31 { i32::MAX } else { 1i32 << zoom };
        let edges = [
            -side,
            side - 1,
            side.wrapping_neg().wrapping_sub(1),
            side,
            0,
            -1,
        ];
        for &x in &edges {
            for &y in &edges {
                cases.push((zoom, [x, y, rand() as i32 >> (rand() % 32)]));
                cases.push((zoom, [rand() as i32 >> (rand() % 32), x, y]));
            }
        }
        for _ in 0..256 {
            let v = |r: u32| r as i32 >> (r % 32);
            cases.push((zoom, [v(rand()), v(rand()), v(rand())]));
        }
    }
    let source = format!("{}\n{KEEPS}", src(SHADER_QUEUE_CORNERS));
    let p = gpu.pipeline(&source, "keeps_cs");
    let mut words = vec![cases.len() as u32];
    for (zoom, c) in &cases {
        words.extend([*zoom, c[0] as u32, c[1] as u32, c[2] as u32, 2, 0]);
    }
    let buf = gpu.buffer(&words, wgpu::BufferUsages::STORAGE);
    gpu.dispatch(
        &p,
        &[(40, buf.as_entire_binding())],
        [(cases.len() as u32).div_ceil(64), 1, 1],
    );
    let out = gpu.read(&buf);
    let (mut kept, mut differ) = (0u32, Vec::new());
    for (i, (zoom, c)) in cases.iter().enumerate() {
        let got = out[1 + i * 6 + 4] == 1;
        let want = node_cache::keeps(*zoom, *c);
        kept += u32::from(want);
        if got != want || out[1 + i * 6 + 5] != node_cache::bucket_of(*zoom, *c) {
            differ.push((*zoom, *c, got));
        }
    }
    eprintln!("{} cases, {kept} kept", cases.len());
    assert!(
        kept > 0 && (kept as usize) < cases.len(),
        "both kinds of case"
    );
    assert!(
        differ.is_empty(),
        "{} differ, first: {:?}",
        differ.len(),
        differ.first()
    );
}
