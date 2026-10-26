// Depth at half resolution, nearest and farthest on a checkerboard so an edge
// keeps both sides; the normal comes from the full-resolution depth.

@group(0) @binding(1) var scene_depth: texture_depth_2d;

fn depth_at(p: vec2i) -> f32 {
  return textureLoad(scene_depth, clamp(p, vec2i(0), vec2i(frame_viewport()) - 1), 0);
}

// From the neighbor with the smaller second difference on each axis, so an
// edge pixel takes its own surface's side (N8AO's computeNormal).
fn view_normal(p: vec2i, res: vec2f, uv: vec2f) -> vec3f {
  let c0 = depth_at(p);
  let l2 = depth_at(p - vec2i(2, 0));
  let l1 = depth_at(p - vec2i(1, 0));
  let r1 = depth_at(p + vec2i(1, 0));
  let r2 = depth_at(p + vec2i(2, 0));
  let b2 = depth_at(p - vec2i(0, 2));
  let b1 = depth_at(p - vec2i(0, 1));
  let t1 = depth_at(p + vec2i(0, 1));
  let t2 = depth_at(p + vec2i(0, 2));
  let dl = abs((2.0 * l1 - l2) - c0);
  let dr = abs((2.0 * r1 - r2) - c0);
  let db = abs((2.0 * b1 - b2) - c0);
  let dt = abs((2.0 * t1 - t2) - c0);
  let ce = view_pos(c0, uv);
  let px = vec2f(1.0 / res.x, 0.0);
  let py = vec2f(0.0, 1.0 / res.y);
  let dpdx = select(view_pos(r1, uv + px) - ce, ce - view_pos(l1, uv - px), dl < dr);
  let dpdy = select(view_pos(t1, uv + py) - ce, ce - view_pos(b1, uv - py), db < dt);
  return normalize(cross(dpdy, dpdx));
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
  let uv = (vec2f(at[chosen]) + 0.5) / frame_viewport();
  return vec4f(d[chosen], view_normal(at[chosen], frame_viewport(), uv));
}
