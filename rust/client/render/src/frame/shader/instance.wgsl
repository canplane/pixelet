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
  // 1 for a cloud's draw, which drifts with the wind; else 0.
  drifts: f32,
};

// Read field by field from vec4 words, never as a struct: a struct here lost
// Safari's WebGPU its GPU process.
@group(1) @binding(0) var<uniform> draw_words: array<vec4f, DRAW_VEC4S>;

// A cloud's draw holds its place on the cloud field's map, which it is drawn
// moved back from by how far the wind has carried the field, Q3 from Q3 first,
// so the difference keeps every step (frame/draw.rs `Position`).
fn draw_translation() -> vec3f {
  let drifts = draw_words[2].w != 0.0;
  let q = bitcast<vec3i>(draw_words[0].xyz);
  let back_q = select(vec2i(0), bitcast<vec2i>(frame_drift_q()), drifts);
  let back_r = select(vec2f(0.0), frame_drift_r(), drifts);
  return from_eye(q - vec3i(back_q, 0), draw_words[2].xyz - vec3f(back_r, 0.0));
}
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
