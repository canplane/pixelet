//! These tests hold the rules that decide where the walks by hand (frame/shader/walk_node.wgsl)
//! stop to pxl-engine's, on each rule's edges and about them. The rules are the zoom asked at a
//! distance, whether a split is narrow at a beam's width, and which nodes a slot of the node
//! cache keeps.

mod gpu;

use pxl_engine::cast::{self, Spacing};
use pxl_engine::{grid, node_cache};
use pxl_render::frame::shader::record::{src, SHADER_QUEUE_CORNERS};

/// The probe's words for one case: the vector's three words, then the four the module writes.
const PROBE_WORDS: usize = 7;

/// A probe that works out the zoom and the narrowness as the walks do (walk_node.wgsl), one
/// thread for each case. It writes the vector's length squared and the zoom asked there over
/// the full range of zooms. It then writes the width of a beam whose rays lie `y` apart in
/// angle at `z` squared from the origin, and whether a split `x` wide is narrow at that width.
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

/// Runs the probe over `cases`, appended to the module of the beam rays' walk, and returns the
/// words it left for each case.
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

/// A vector's length squared, summed in the order of pxl-engine's `squared` (cast.rs), which
/// the walks keep.
fn squared(v: [f32; 3]) -> f32 {
    v[0] * v[0] + v[1] * v[1] + v[2] * v[2]
}

fn next_up(x: f32) -> f32 {
    f32::from_bits(x.to_bits() + 1)
}

fn next_down(x: f32) -> f32 {
    f32::from_bits(x.to_bits() - 1)
}

/// How many last places a device's distance squared may lie off the Rust's. A device may fuse
/// a product into the sum (`squared`), which skips one rounding.
const FUSED_ULPS: u32 = 1;

/// On a split distance squared, `2304 · 4^k`, and on the floats either side of it, the zoom the
/// walks ask must be `grid::cell_zoom_at`'s. The vectors lie along an axis, a little off an
/// axis, and across all three axes. A device's distance squared may lie up to `FUSED_ULPS` off
/// the Rust's, and its zoom must then be the one at its own distance. The test counts each case
/// that took the other zoom that way.
#[test]
fn hand_zoom_on_a_split_distance() {
    let Some(gpu) = gpu::Gpu::new() else {
        eprintln!("no adapter: skipped");
        return;
    };
    // For each split distance, the cases are the vectors whose squares, summed as the Rust sums
    // them, give the distance squared or the float either side of it.
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
        // A second component whose square is one ulp of the distance squared, so the sum is the
        // float over it.
        put([d, pow2(k - 6), 0.0]);
        put([0.0, -pow2(k - 6), d]);
        // The float under the distance squared: the next f32 under the distance squares to it.
        put([next_down(d), 0.0, 0.0]);
        put([0.0, 0.0, next_down(d)]);
        // Vectors across all three axes, with X stepped over the floats about the value whose
        // sum comes to the distance squared.
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
        // The case lies on a zoom's edge when a float either side of the Rust's distance
        // squared asks another zoom.
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

/// A split exactly `NARROW` beam widths wide, where `side² = 8 · width²`, must be narrow as
/// `cast::narrow` says. The cases scale that split by powers of two, take the width from a
/// spacing at a distance squared (`Spacing::width`), and step the side to the floats either
/// side.
#[test]
fn hand_narrow_on_its_edge() {
    let Some(gpu) = gpu::Gpu::new() else {
        eprintln!("no adapter: skipped");
        return;
    };
    // `w` is the f32 nearest `1 / (2√2)`. Its square, rounded and times 8, lands one place over
    // 1, so a side of 1 lies on the bound.
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
            // An angle and a distance squared whose width is exactly `w · side`.
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
        // Counts the cases that the bound taken as `NARROW` squared in f32, a place under 8,
        // would decide otherwise.
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

/// How many last places a device's beam width may lie off the Rust's. Only a GPU walks a beam,
/// so the GPU alone decides this rule (walk_node.wgsl `spacing_width`). WGSL takes a square
/// root's accuracy from `1 / inverseSqrt`: the division may be 2.5 places off and the inverse
/// root 2. The product by the angle rounds once more.
const WIDTH_ULPS: u32 = 5;

/// At a distance squared whose root no float holds exactly, the device's width must lie within
/// `WIDTH_ULPS` of `Spacing::width`'s. A split `NARROW` of the Rust's widths wide, or a few
/// floats either side, must be narrow as `cast::narrow` says of the device's own width. The
/// test counts each width off the Rust's, and each split whose narrowness that parted from the
/// Rust's.
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
        // A distance from 2^-4 to 2^24 units, and an angle as wide as the eye's lattice's or
        // finer.
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

/// Two to the `k`, exactly, for a `k` within a normal float's exponents.
fn pow2(k: i32) -> f32 {
    f32::from_bits(((127 + k) as u32) << 23)
}

/// A probe of whether a slot keeps a node as the walks ask it, and of the bucket it is kept in
/// (walk_node.wgsl `keeps`, `bucket_of`), one thread for each case. A case's words are the
/// node's zoom and its index, then the two words the module writes.
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

/// The walks must keep a node exactly where pxl-engine's `node_cache::keeps` does, in the
/// bucket `node_cache::bucket_of` names. The cases cover every zoom up to two past the finest.
/// On each axis they take the indices at and across the edges of twice the world's span at that
/// zoom, and noise besides.
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
