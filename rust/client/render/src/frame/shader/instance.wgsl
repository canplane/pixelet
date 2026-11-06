// A decor draw's instances, as the model tier (decor.wgsl) and the
// impostors (impostor.wgsl) both read them: the draw's uniform
// (gpu/pass/model.rs `put_instances`) and the decor's instances, `instances`,
// a lane a field; the n-th instance drawn is the one the list `order` names.

struct Draw {
  // The position's Q3 and its remainder (frame/draw.rs `Position`).
  q: vec3i,
  off: u32,
  stride: u32,
  opacity: f32,
  // The impostor's atlas layer; the model tier's padding.
  layer: u32,
  idx: u32,
  r: vec3f,
  _p: f32,
};

// Read field by field from vec4 words, never as a struct: a struct here lost
// Safari's WebGPU its GPU process.
@group(1) @binding(0) var<uniform> draw_words: array<vec4f, DRAW_VEC4S>;

fn draw_translation() -> vec3f { return from_eye(bitcast<vec3i>(draw_words[0].xyz), draw_words[2].xyz); }
fn draw_off() -> u32 { return bitcast<u32>(draw_words[0].w); }
fn draw_stride() -> u32 { return bitcast<u32>(draw_words[1].x); }
fn draw_opacity() -> f32 { return draw_words[1].y; }
fn draw_layer() -> u32 { return bitcast<u32>(draw_words[1].z); }
fn draw_idx() -> u32 { return bitcast<u32>(draw_words[1].w); }

@group(3) @binding(0) var<storage, read> instances: array<u32>;
// The n-th instance a draw draws is the one the n-th word from its list's first
// (`draw_idx`) names: the instances' own list, or in a frame through the gate the list of
// those that meet its claimed tiles (decor_gate.wgsl).
@group(3) @binding(1) var<storage, read> order: array<u32>;

// Instance `i`'s word in lane `k`.
fn lane(k: u32, i: u32) -> f32 {
  return bitcast<f32>(instances[draw_off() + k * draw_stride() + i]);
}
