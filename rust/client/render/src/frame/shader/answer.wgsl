// pxl-render/answer.wgsl

// A node's answer (pxl-engine answer.rs), as the world's query gives it (world.wgsl) and every
// walk reads it (walk.wgsl), off the node cache alone or asked.

/// What is in a node (`world::node`) and what it hands its children.
struct Answer {
  kind: u32,
  asset: vec2<u32>,
  handed: vec4<u32>,
}

/// A split's box, the steps its content lies in (`answer::box_of`): its first step on each axis,
/// and the step past its last.
fn box_first(w: vec2<u32>) -> vec3<u32> {
  let m = BOX_STEPS - 1u;
  return vec3<u32>(w.x & m, (w.x >> (2u * BOX_BITS)) & m, (w.y >> BOX_BITS) & m);
}

fn box_past(w: vec2<u32>) -> vec3<u32> {
  let m = BOX_STEPS - 1u;
  return vec3<u32>((w.x >> BOX_BITS) & m, w.y & m, (w.y >> (2u * BOX_BITS)) & m) + 1u;
}
