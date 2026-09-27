// What the screen-space occlusion's four passes share: the camera as they
// read it, a full-screen triangle, and the view-space point and normal a
// pixel's depth gives back. The passes are N8AO 1.10.1 (n8ao's
// DepthDownSample, EffectShader, PoissionBlur and EffectCompositer, CC0)
// written for WGSL and this renderer's reversed depth, at the numbers the
// three.js build ran it at until 2026-09-14 — the "performance" quality
// at half resolution: 8 hemisphere samples, a 4-tap denoise run twice,
// a radius of 1.5 units, an intensity of 1.2 — so that build's contact
// shadow can be put beside the corner occlusion a chunk shades itself
// with. Everything is in view space: N8AO's getWorldPos multiplied by
// the projection's inverse alone and so was view space too, under the
// wrong name.

struct Ao {
  // View to WebGPU clip, and clip back to view.
  proj: mat4x4f,
  inv_proj: mat4x4f,
  // The frame's pixels, and the half-resolution passes' pixels.
  full: vec2f,
  half: vec2f,
  // The frame's fog band along the view axis, which lifts the occlusion
  // where the fog would hide it, as the compositer did.
  fog_near: f32,
  fog_far: f32,
  _p0: f32,
  _p1: f32,
};

// Held as vec4 words and read back by `ao()`: what the host and a shader
// share has no room inside it for a driver to lay out its own way
// (issue/20260927-a-matrix-the-driver-reads-its-own-way.md).
@group(0) @binding(0) var<uniform> ao_words: array<vec4f, 10>;

fn ao() -> Ao {
  return Ao(
    mat4x4f(ao_words[0], ao_words[1], ao_words[2], ao_words[3]),
    mat4x4f(ao_words[4], ao_words[5], ao_words[6], ao_words[7]),
    ao_words[8].xy,
    ao_words[8].zw,
    ao_words[9].x,
    ao_words[9].y,
    ao_words[9].z,
    ao_words[9].w,
  );
}

const AO_RADIUS: f32 = 1.5;
const AO_DISTANCE_FALLOFF: f32 = 1.0;
const AO_INTENSITY: f32 = 1.2;
const AO_SAMPLES: i32 = 8;
const AO_DENOISE_SAMPLES: i32 = 4;
// N8AO's denoiseRadius of 12 texels, halved as its half-resolution run
// halves it.
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

// The view-space point at depth `d` (reversed, as the buffer holds it)
// seen through the pixel at `uv`, y down as a texture has it.
fn view_pos(d: f32, uv: vec2f) -> vec3f {
  let p = ao().inv_proj * vec4f(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, d, 1.0);
  return p.xyz / p.w;
}

// The distance along the view axis at depth `d`.
fn view_dist(d: f32) -> f32 {
  let p = ao().inv_proj * vec4f(0.0, 0.0, d, 1.0);
  return -p.z / p.w;
}

// The view-space normal at pixel `p` of a depth image `res` pixels across,
// from the neighbor on each axis whose second difference is the smaller —
// the side that lies on the same surface, where the pixel sits on an edge
// — as N8AO's computeNormal took it. `depth_at` is the including pass's,
// over the depth it reads. The cross is taken in the order that points the
// normal toward the eye with y running down the image.
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
