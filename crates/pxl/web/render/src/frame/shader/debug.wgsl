// Debug gizmos as one line list with no depth test, so a gizmo shows through the ground.

struct Draw {
  translation: vec3f,
  _p: f32,
};
// Read field by field from vec4 words, never as a struct: a struct here lost
// Safari's WebGPU its GPU process.
@group(1) @binding(0) var<uniform> draw_words: array<vec4f, 1>;

fn draw_translation() -> vec3f { return draw_words[0].xyz; }

struct VertexOut {
  @builtin(position) clip: vec4f,
  @location(0) color: vec3f,
};

@vertex
fn vs(@location(0) position: vec3f, @location(1) color: vec3f) -> VertexOut {
  var o: VertexOut;
  o.clip = frame_view_proj() * vec4f(on_globe(position + draw_translation()), 1.0);
  o.color = color;
  return o;
}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  return vec4f(to_srgb(i.color), 1.0);
}
