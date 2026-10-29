// Shared by the AO passes: N8AO 1.10.1 (CC0) ported to WGSL and reversed depth,
// at its "performance" settings. Everything is in view space.

// Read field by field from vec4 words, never as a struct: a struct here lost
// Safari's WebGPU its GPU process. The screen's size and the fog are the frame's.
@group(0) @binding(6) var<uniform> ao_words: array<vec4f, AO_VEC4S>;

// The blue-noise texture's side less one; a pixel reads it tiled.
const AO_NOISE_MASK: i32 = i32(AO_NOISE_SIDE) - 1;

struct VertexOut {
  @builtin(position) clip: vec4f,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32) -> VertexOut {
  let p = array<vec2f, 3>(vec2f(-1.0, -1.0), vec2f(3.0, -1.0), vec2f(-1.0, 3.0));
  var o: VertexOut;
  o.clip = vec4f(p[vi], 0.0, 1.0);
  return o;
}

// Reversed depth `d`; `uv` y down.
fn view_pos(d: f32, uv: vec2f) -> vec3f {
  let p = ao_inv_proj() * vec4f(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, d, 1.0);
  return p.xyz / p.w;
}

fn view_dist(d: f32) -> f32 {
  let p = ao_inv_proj() * vec4f(0.0, 0.0, d, 1.0);
  return -p.z / p.w;
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
