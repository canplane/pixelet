// pxl-render/answer.wgsl

// A node's answer, laid out as pxl-engine answer.rs defines it. The world's query gives it
// (world.wgsl), and every walk reads it (walk.wgsl), whether the walk finds it in the node cache
// or asks the world for it.

/// What a node holds (pxl-engine `world::node`), and what it hands down to its children
/// (`world::Inheritance`).
struct Answer {
  kind: u32,
  asset: vec2<u32>,
  inheritance: vec4<u32>,
}

/// A split's asset words `w` hold its box, the steps its content may lie in (pxl-engine
/// `answer::box_of`). `box_first` reads the box's first step on each axis, and `box_past` the
/// step past its last.
fn box_first(w: vec2<u32>) -> vec3<u32> {
  let m = BOX_STEPS - 1u;
  return vec3<u32>(w.x & m, (w.x >> (2u * BOX_BITS)) & m, (w.y >> BOX_BITS) & m);
}

fn box_past(w: vec2<u32>) -> vec3<u32> {
  let m = BOX_STEPS - 1u;
  return vec3<u32>((w.x >> BOX_BITS) & m, w.y & m, (w.y >> (2u * BOX_BITS)) & m) + 1u;
}
