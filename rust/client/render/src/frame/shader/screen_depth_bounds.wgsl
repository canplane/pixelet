// pxl-render/frame/shader/screen_depth_bounds.wgsl

// The world's depth a frame drew, the least and the most of each square of
// the screen `SIDE` drawn pixels a side, reversed (gpu/depth_bounds.rs):
// where the world stands under the screen, whatever stands there, ground,
// decor or an actor, which the host bounds what a moved caster shades by.
// Only what was drawn counts; a square of none holds 2 and 0.

@group(0) @binding(0) var depth: texture_depth_2d;
@group(0) @binding(1) var<storage, read_write> bounds: array<vec2f>;
@group(0) @binding(2) var<uniform> within: vec4u;

const SIDE: u32 = 32u;

@compute @workgroup_size(8, 8, 1)
fn depth_bounds_cs(@builtin(global_invocation_id) id: vec3u) {
  let size = textureDimensions(depth);
  let across = (size + SIDE - 1u) / SIDE;
  let t = id.xy + within.xy;
  if (any(t >= across) || any(id.xy >= within.zw)) {
    return;
  }
  var b = vec2f(2.0, 0.0);
  for (var y = 0u; y < SIDE; y++) {
    for (var x = 0u; x < SIDE; x++) {
      let p = t * SIDE + vec2u(x, y);
      if (all(p < size)) {
        let d = textureLoad(depth, vec2i(p), 0);
        if (d > 0.0) {
          b = vec2f(min(b.x, d), max(b.y, d));
        }
      }
    }
  }
  bounds[t.y * across.x + t.x] = b;
}
