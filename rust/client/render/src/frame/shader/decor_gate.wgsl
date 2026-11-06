// pxl-render/frame/shader/decor_gate.wgsl

// What a frame through the gate draws of decor generated (gpu/generator.rs
// `Generator::gate`): of each region's instances, those whose ball (decor_ball.wgsl) meets the
// box of the claimed tiles (claimed_box.wgsl) as the frame sees it, listed at the region's
// own place in a list beside the instances (`gated`), with the region's args, their count
// the instances listed. The draws read the list in place of the instances' own
// (instance.wgsl `order`); a pixel past the claimed tiles keeps the last frame's, so an
// instance whose ball meets none of them draws none of it.

@group(0) @binding(1) var<storage, read> uniforms: array<u32>;
@group(0) @binding(2) var<storage, read> args: array<u32>;
@group(0) @binding(3) var<storage, read> instances: array<u32>;
@group(0) @binding(4) var<storage, read_write> gated: array<u32>;
@group(0) @binding(5) var<storage, read_write> gated_args: array<atomic<u32>>;
@group(0) @binding(6) var<storage, read> claimed_box: array<u32, 4>;
/// Each region's count, then its end: its first slot as the args pass laid it
/// (decor_args.wgsl), past its last once compacted (decor_compact.wgsl).
@group(0) @binding(7) var<storage, read> regions: array<u32>;
@group(0) @binding(31) var<uniform> tiles: Tiles;

/// Region `r`'s draw words: the lanes' base and stride, its list's first word.
fn region_word(r: u32, k: u32) -> u32 {
  return uniforms[r * UNIFORM_WORDS + k];
}

/// Each region's args as generated, none of its instances listed yet: a thread a region,
/// before `decor_gate_cs`.
@compute @workgroup_size(64, 1, 1)
fn decor_gate_args_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x >= REGIONS) {
    return;
  }
  for (var k = 0u; k < ARGS_WORDS; k += 1u) {
    atomicStore(&gated_args[id.x * ARGS_WORDS + k], select(args[id.x * ARGS_WORDS + k], 0u, k == 1u));
  }
}

/// Whether a ball at `c` of radius `r` lies on the inner side of the plane `p` of clip space,
/// where `p · (x, y, z, w) >= 0`.
fn inside(p: vec4<f32>, c: vec3<f32>, r: f32) -> bool {
  return dot(p.xyz, c) + p.w >= -r * length(p.xyz);
}

/// A thread a place of the instances' lists, laid region after region.
@compute @workgroup_size(64, 1, 1)
fn decor_gate_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  // The regions' ends, as the compaction leaves them (decor_compact.wgsl): the region whose
  // list holds the place is the first that ends past it.
  let last = REGIONS - 1u;
  if (id.x >= regions[REGIONS + last]) {
    return;
  }
  var first = 0u;
  var past = last;
  while (first < past) {
    let mid = (first + past) / 2u;
    if (regions[REGIONS + mid] > id.x) {
      past = mid;
    } else {
      first = mid + 1u;
    }
  }
  let region = first;
  let at = DRAWN_COLUMNS * region_word(0u, 4u) + id.x;
  let b = claimed_box;
  if (b[0] > b[2] || b[1] > b[3]) {
    return;
  }
  let slot = instances[at];
  let off = region_word(region, 3u);
  let stride = region_word(region, 4u);
  // The anchor's Q3 against the eye's, as the draw places it (instance.wgsl
  // `draw_translation`): decor's anchor is on Q3, so this is the f64
  // difference narrowed once, and the gate decides on what is drawn.
  var v = from_eye(
    vec3<i32>(bitcast<i32>(region_word(region, 0u)), bitcast<i32>(region_word(region, 1u)), bitcast<i32>(region_word(region, 2u))),
    vec3<f32>(bitcast<f32>(region_word(region, 8u)), bitcast<f32>(region_word(region, 9u)), bitcast<f32>(region_word(region, 10u))),
  );
  for (var k = 0u; k < 3u; k += 1u) {
    v[k] += bitcast<f32>(instances[off + k * stride + slot]);
  }
  // The claimed tiles' pixels, grown a pixel each way for the jitter a draw is moved by
  // (frame.wgsl `jittered`), as clip space's planes through the eye.
  let side = tiles.size.w;
  let size = tiles.size.xy;
  let lo = vec2<f32>(f32(b[0]), f32(b[1])) * side - vec2<f32>(1.0);
  let hi = min(vec2<f32>(f32(b[2] + 1u), f32(b[3] + 1u)) * side, size) + vec2<f32>(1.0);
  let x = vec2<f32>(lo.x, hi.x) / size.x * 2.0 - 1.0;
  let y = 1.0 - vec2<f32>(lo.y, hi.y) / size.y * 2.0;
  let m = frame_view_proj();
  let row = array<vec4<f32>, 4>(
    vec4<f32>(m[0][0], m[1][0], m[2][0], m[3][0]),
    vec4<f32>(m[0][1], m[1][1], m[2][1], m[3][1]),
    vec4<f32>(m[0][2], m[1][2], m[2][2], m[3][2]),
    vec4<f32>(m[0][3], m[1][3], m[2][3], m[3][3]),
  );
  let c = decor_middle(v);
  let r = decor_radius();
  if (inside(row[0] - x.x * row[3], c, r) && inside(x.y * row[3] - row[0], c, r)
    && inside(y.x * row[3] - row[1], c, r) && inside(row[1] - y.y * row[3], c, r)) {
    let n = atomicAdd(&gated_args[region * ARGS_WORDS + 1u], 1u);
    gated[region_word(region, UNIFORM_IDX) + n] = slot;
  }
}
