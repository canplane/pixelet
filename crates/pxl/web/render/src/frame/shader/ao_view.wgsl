// Shared by the AO passes: N8AO 1.10.1 (CC0) ported to WGSL and reversed depth,
// at its "performance" settings. Everything is in view space.

struct Ao {
  proj: mat4x4f,
  inv_proj: mat4x4f,
  full: vec2f,
  half: vec2f,
  // Lifts the occlusion where fog would hide it, as N8AO's compositer did.
  fog_near: f32,
  fog_far: f32,
  _p0: f32,
  _p1: f32,
};

// Read field by field from vec4 words, never as a struct: a struct here lost
// Safari's WebGPU its GPU process.
@group(0) @binding(0) var<uniform> ao_words: array<vec4f, 10>;

fn ao_proj() -> mat4x4f { return mat4x4f(ao_words[0], ao_words[1], ao_words[2], ao_words[3]); }
fn ao_inv_proj() -> mat4x4f { return mat4x4f(ao_words[4], ao_words[5], ao_words[6], ao_words[7]); }
fn ao_full() -> vec2f { return ao_words[8].xy; }
fn ao_half() -> vec2f { return ao_words[8].zw; }
fn ao_fog_near() -> f32 { return ao_words[9].x; }
fn ao_fog_far() -> f32 { return ao_words[9].y; }

const AO_RADIUS: f32 = 1.5;
const AO_DISTANCE_FALLOFF: f32 = 1.0;
const AO_INTENSITY: f32 = 1.2;
const AO_SAMPLES: i32 = 8;
const AO_DENOISE_SAMPLES: i32 = 4;
// N8AO's 12-texel denoise radius, halved for half resolution.
const AO_DENOISE_RADIUS: f32 = 6.0;
// The blue-noise texture's side; a pixel reads it tiled.
const AO_NOISE_MASK: i32 = 127;

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
