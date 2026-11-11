// How a decor draw reads its instances, for the model tier (decor.wgsl) and the
// impostors (impostor.wgsl) alike. The draw has a uniform of its own (word.rs
// `instance`), and the decor's instances lie in `instances`.

// The uniform is read field by field from vec4 words and is never declared as a
// struct. Declared as a struct here, it makes Safari's WebGPU lose its GPU
// process.
@group(1) @binding(0) var<uniform> instance_words: array<vec4f, DRAW_VEC4S>;

// Where the draw stands from the eye. A draw that drifts is a cloud's: it holds
// its place on the cloud field's map, and it is drawn moved back by how far the
// wind has carried the field. The Q3 steps are subtracted before the
// remainders, so the difference keeps every step (gpu/draw.rs `Position`).
fn draw_translation() -> vec3f {
  let drifts = instance_drifts() != 0.0;
  let back_q = select(vec2i(0), bitcast<vec2i>(frame_drift_q()), drifts);
  let back_r = select(vec2f(0.0), frame_drift_r(), drifts);
  return from_eye(instance_q() - vec3i(back_q, 0), instance_r() - vec3f(back_r, 0.0));
}

@group(3) @binding(0) var<storage, read> instances: array<u32>;
// The n-th instance a draw draws is the one named by the n-th word from the
// first of its list (`instance_idx`). The list is the instances' own, or, in a
// frame drawn through the gate, the list of the instances that meet its damaged
// tiles (decor_cull.wgsl).
@group(3) @binding(1) var<storage, read> order: array<u32>;

// The value of field `k` for instance `i`. Each field is a lane of `instance_stride` words,
// one word for each instance.
fn lane(k: u32, i: u32) -> f32 {
  return bitcast<f32>(instances[instance_off() + k * instance_stride() + i]);
}
