// pxl-render/frame/shader/screen_bound.wgsl

// The depth's bound the reach reads (gpu/reach.rs, screen_reach.wgsl): each
// square of the screen `BOUND_STEP` pixels a side, its least and most depth
// the world's pass drew, reversed, the sky left out (a square of sky alone
// holds 2 and 0). Set after the world's pass in the tiles it drew this frame
// (screen_tiles.wgsl), every one where it drew every one, so it holds what
// the depth holds while the eye stands: a workgroup a tile, a thread a
// square.

@group(0) @binding(0) var depth: texture_depth_2d;
@group(0) @binding(1) var<storage, read_write> bounds: array<vec2f>;

// A square's pixels a side: a thread's of a tile's eight a side (pxl-spec-core
// `screen::TILE`, 32), the reach's (`REACH_STEP`).
const BOUND_STEP: u32 = 4u;

@compute @workgroup_size(8, 8, 1)
fn screen_bound_cs(
  @builtin(workgroup_id) wg: vec3u,
  @builtin(local_invocation_id) l: vec3u,
) {
  let tile = wg.xy;
  if (any(tile >= tiles.count.xy) || !tile_claimed(tile.y * tiles.count.x + tile.x)) {
    return;
  }
  let size = vec2u(textureDimensions(depth));
  let at = tile * tile_side() + l.xy * BOUND_STEP;
  if (any(at >= size)) {
    return;
  }
  var b = vec2f(2.0, 0.0);
  for (var k = 0u; k < BOUND_STEP * BOUND_STEP; k++) {
    let p = at + vec2u(k % BOUND_STEP, k / BOUND_STEP);
    if (all(p < size)) {
      let d = textureLoad(depth, vec2i(p), 0);
      if (d > 0.0) {
        b = vec2f(min(b.x, d), max(b.y, d));
      }
    }
  }
  let across = (size.x + BOUND_STEP - 1u) / BOUND_STEP;
  let s = at / BOUND_STEP;
  bounds[s.y * across + s.x] = b;
}
