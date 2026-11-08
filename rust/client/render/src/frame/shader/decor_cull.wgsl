// pxl-render/frame/shader/decor_cull.wgsl

// Picks what a frame through the gate draws of the decor generated (gpu/generator.rs
// `Generator::cull`). Of each region's instances, it lists those whose ball (decor_ball.wgsl)
// meets the box of the claimed tiles (damage_rect.wgsl) as the frame sees it. They are listed
// at the region's own place in a list beside the instances (`culled`), and the region's args
// count the instances listed. The draws read this list in place of the instances' own
// (instance.wgsl `order`). A pixel past the claimed tiles keeps the last frame's color, so an
// instance whose ball meets none of them has nothing to draw.

@group(0) @binding(1) var<storage, read> uniforms: array<u32>;
@group(0) @binding(2) var<storage, read> args: array<u32>;
@group(0) @binding(3) var<storage, read> instances: array<u32>;
@group(0) @binding(4) var<storage, read_write> culled: array<u32>;
@group(0) @binding(5) var<storage, read_write> culled_args: array<atomic<u32>>;
@group(0) @binding(6) var<storage, read> damage_rect: array<u32, 4>;
/// Each region's count, then its end. The end holds the region's first slot as the args pass
/// laid it (decor_args.wgsl), and the slot past its last once compacted (decor_compact.wgsl).
@group(0) @binding(7) var<storage, read> regions: array<u32>;
@group(0) @binding(31) var<uniform> tiles: TileGrid;

/// Word `k` of region `r`'s draw uniform, which holds the lanes' base and stride and the first
/// word of the region's list.
fn region_word(r: u32, k: u32) -> u32 {
  return uniforms[r * UNIFORM_WORDS + k];
}

/// Copies each region's args as generated, with none of its instances listed yet, one thread a
/// region, before `decor_cull_cs` runs.
@compute @workgroup_size(64, 1, 1)
fn decor_cull_args_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x >= REGIONS) {
    return;
  }
  for (var k = 0u; k < ARGS_WORDS; k += 1u) {
    atomicStore(&culled_args[id.x * ARGS_WORDS + k], select(args[id.x * ARGS_WORDS + k], 0u, k == 1u));
  }
}

/// Whether a ball at `c` of radius `r` lies on the inner side of the plane `p` of clip space,
/// where `p · (x, y, z, w) >= 0`.
fn inside(p: vec4<f32>, c: vec3<f32>, r: f32) -> bool {
  return dot(p.xyz, c) + p.w >= -r * length(p.xyz);
}

/// Lists each instance whose ball meets the claimed tiles, one thread a place of the
/// instances' lists, which lie region after region.
@compute @workgroup_size(64, 1, 1)
fn decor_cull_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  // The regions' ends lie as the compaction leaves them (decor_compact.wgsl), so the region
  // whose list holds the place is the first that ends past it.
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
  let b = damage_rect;
  if (b[0] > b[2] || b[1] > b[3]) {
    return;
  }
  let slot = instances[at];
  let off = region_word(region, 3u);
  let stride = region_word(region, 4u);
  // The anchor is placed against the eye in Q3, as the draw places it (instance.wgsl
  // `draw_translation`). Decor's anchor lies on Q3, so this is the f64 difference narrowed
  // once, and the gate decides on the positions that are drawn.
  var v = from_eye(
    vec3<i32>(bitcast<i32>(region_word(region, 0u)), bitcast<i32>(region_word(region, 1u)), bitcast<i32>(region_word(region, 2u))),
    vec3<f32>(bitcast<f32>(region_word(region, 8u)), bitcast<f32>(region_word(region, 9u)), bitcast<f32>(region_word(region, 10u))),
  );
  for (var k = 0u; k < 3u; k += 1u) {
    v[k] += bitcast<f32>(instances[off + k * stride + slot]);
  }
  // The claimed tiles' pixels, grown a pixel each way for the jitter that moves a draw
  // (frame.wgsl `jittered`), taken as planes of clip space through the eye.
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
    let n = atomicAdd(&culled_args[region * ARGS_WORDS + 1u], 1u);
    culled[region_word(region, UNIFORM_IDX) + n] = slot;
  }
}
