// A decor draw's instances, as the model tier (decor.wgsl) and the
// impostors (impostor.wgsl) both read them: the draw's uniform (word.rs
// `instance`) and the decor's instances, `instances`, a lane a field; the
// n-th instance drawn is the one the list `order` names.

// Read field by field from vec4 words, never as a struct: a struct here lost
// Safari's WebGPU its GPU process.
@group(1) @binding(0) var<uniform> instance_words: array<vec4f, DRAW_VEC4S>;

// A cloud's draw holds its place on the cloud field's map, which it is drawn
// moved back from by how far the wind has carried the field, Q3 from Q3 first,
// so the difference keeps every step (gpu/draw.rs `Position`).
fn draw_translation() -> vec3f {
  let drifts = instance_drifts() != 0.0;
  let back_q = select(vec2i(0), bitcast<vec2i>(frame_drift_q()), drifts);
  let back_r = select(vec2f(0.0), frame_drift_r(), drifts);
  return from_eye(instance_q() - vec3i(back_q, 0), instance_r() - vec3f(back_r, 0.0));
}

@group(3) @binding(0) var<storage, read> instances: array<u32>;
// The n-th instance a draw draws is the one the n-th word from its list's first
// (`instance_idx`) names: the instances' own list, or in a frame through the gate the list of
// those that meet its claimed tiles (decor_cull.wgsl).
@group(3) @binding(1) var<storage, read> order: array<u32>;

// Instance `i`'s word in lane `k`.
fn lane(k: u32, i: u32) -> f32 {
  return bitcast<f32>(instances[instance_off() + k * instance_stride() + i]);
}
