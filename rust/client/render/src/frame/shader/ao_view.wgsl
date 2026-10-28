// Shared by the AO passes: N8AO 1.10.1 (CC0) ported to WGSL and reversed depth,
// at its "performance" settings with four hemisphere samples, not eight:
// the sample pass is most of its cost. Everything is in view space.

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
