// What every pass of the occlusion and its apply share. The occlusion is N8AO
// 1.10.1 (CC0), ported to WGSL and to reversed depth, at its "performance"
// settings with four hemisphere samples. Every position here is in view space.

// The occlusion's words, read field by field from vec4s and never built back
// into a struct: a struct built from them crashed the GPU process of Safari's
// WebGPU. The screen's size and the fog are read from the frame's words.
@group(0) @binding(6) var<uniform> ao_words: array<vec4f, AO_VEC4S>;

// A pixel reads the blue noise tiled, its coordinates masked by the noise's side less one,
// which holds only while that side is a power of two.
const AO_NOISE_MASK: i32 = i32(AO_NOISE_SIDE) - 1;

// The view-space position at reversed depth `d` and screen coordinates `uv`,
// whose y points down.
fn view_pos(d: f32, uv: vec2f) -> vec3f {
  let p = ao_inv_proj() * vec4f(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, d, 1.0);
  return p.xyz / p.w;
}

fn view_dist(d: f32) -> f32 {
  let p = ao_inv_proj() * vec4f(0.0, 0.0, d, 1.0);
  return -p.z / p.w;
}
