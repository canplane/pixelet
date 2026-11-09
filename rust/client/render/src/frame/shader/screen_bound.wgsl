// pxl-render/frame/shader/screen_bound.wgsl

// Sets the depth bound the propagation reads (gpu/propagate.rs, screen_propagate.wgsl): for
// each square of the screen `BOUND_STEP` pixels a side, the least and the most reversed depth
// the world's pass drew there. The sky, at depth 0, is left out, so a square of sky alone
// holds 2 and 0, an empty range. The bound stays on the GPU across frames and is set after
// each world pass in the tiles that pass drew (screen_tile.wgsl), so it matches the depth
// while the eye stands still.

@group(0) @binding(0) var depth: texture_depth_2d;
@group(0) @binding(1) var<storage, read_write> bounds: array<vec2f>;

// A workgroup covers one screen tile, and each of its threads bounds one square of the tile
// (word.rs `BOUND_SQUARES`).
@compute @workgroup_size(BOUND_SQUARES, BOUND_SQUARES, 1)
fn screen_bound_cs(
  @builtin(workgroup_id) wg: vec3u,
  @builtin(local_invocation_id) l: vec3u,
) {
  let tile = wg.xy;
  if (any(tile >= tiles.cnt.xy) || !tile_in_damage(tile.y * tiles.cnt.x + tile.x)) {
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
