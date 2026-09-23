// The dev gizmos: the module's line segments (debug.rs) drawn as one line
// list over everything, with no depth test — a gizmo marks a place whether
// or not the ground is in front of it. A pixel wide, which is what a line
// list is; the labels are the glue's DOM.

struct Draw {
  translation: vec3f,
  _p: f32,
};
@group(1) @binding(0) var<uniform> draw: Draw;

struct VertexOut {
  @builtin(position) clip: vec4f,
  @location(0) color: vec3f,
};

@vertex
fn vs(@location(0) position: vec3f, @location(1) color: vec3f) -> VertexOut {
  var o: VertexOut;
  o.clip = frame.view_proj * vec4f(on_globe(position + draw.translation), 1.0);
  o.color = color;
  return o;
}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  return vec4f(to_srgb(i.color), 1.0);
}
