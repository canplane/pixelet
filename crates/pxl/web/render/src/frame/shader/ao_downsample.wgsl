// The frame's depth at half resolution with the normal beside it: of each
// 2x2 of depths, the nearest on one pixel of a checkerboard and the
// farthest on the other, so an edge keeps both of its sides and neither
// side's depth is invented; the normal is taken from the full-resolution
// depth around the pixel that was kept. Into four floats: the depth, then
// the normal.

@group(0) @binding(1) var scene_depth: texture_depth_2d;

fn depth_at(p: vec2i) -> f32 {
  return textureLoad(scene_depth, clamp(p, vec2i(0), vec2i(ao().full) - 1), 0);
}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  let hp = vec2i(i.clip.xy);
  let fp = hp * 2;
  var at = array<vec2i, 4>(fp, fp + vec2i(1, 0), fp + vec2i(0, 1), fp + vec2i(1, 1));
  var d = array<f32, 4>(depth_at(at[0]), depth_at(at[1]), depth_at(at[2]), depth_at(at[3]));
  // Reversed depth: the greater is the nearer.
  let nearest = max(max(d[0], d[1]), max(d[2], d[3]));
  let farthest = min(min(d[0], d[1]), min(d[2], d[3]));
  let kept = select(nearest, farthest, (hp.x + hp.y) % 2 == 1);
  var chosen = 0;
  for (var k = 0; k < 4; k++) {
    if (d[k] == kept) {
      chosen = k;
      break;
    }
  }
  let uv = (vec2f(at[chosen]) + 0.5) / ao().full;
  return vec4f(d[chosen], view_normal(at[chosen], ao().full, uv));
}
