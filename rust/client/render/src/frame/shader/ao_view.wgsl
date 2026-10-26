// Shared by the AO passes: N8AO 1.10.1 (CC0) ported to WGSL and reversed depth,
// at its "performance" settings with four hemisphere samples, not eight:
// the sample pass is most of its cost. Everything is in view space.

struct Ao {
  proj: mat4x4f,
  inv_proj: mat4x4f,
  half: vec2f,
  _p0: vec2f,
};

// Read field by field from vec4 words, never as a struct: a struct here lost
// Safari's WebGPU its GPU process. The screen's size and the fog are the frame's.
@group(0) @binding(6) var<uniform> ao_words: array<vec4f, AO_VEC4S>;

fn ao_proj() -> mat4x4f { return mat4x4f(ao_words[0], ao_words[1], ao_words[2], ao_words[3]); }
fn ao_inv_proj() -> mat4x4f { return mat4x4f(ao_words[4], ao_words[5], ao_words[6], ao_words[7]); }
fn ao_half() -> vec2f { return ao_words[8].xy; }

const AO_RADIUS: f32 = 1.5;
const AO_DISTANCE_FALLOFF: f32 = 1.0;
const AO_INTENSITY: f32 = 1.2;
const AO_SAMPLES: i32 = 4;
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
