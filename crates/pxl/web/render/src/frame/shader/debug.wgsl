// The debug gizmos: the module's line segments (frame/draw.rs) drawn as one line
// list over everything, with no depth test — a gizmo marks a place whether
// or not the ground is in front of it. A pixel wide, which is what a line
// list is; the labels are the glue's DOM.

struct Draw {
  translation: vec3f,
  _p: f32,
};
// Held as vec4 words, each field read from its own by `draw_<field>()`:
// what the host and a shader share has no room inside it for a driver to
// lay out its own way (issue/20260927-a-matrix-the-driver-reads-its-own-way.md),
// and nothing builds the struct back, with which Safari's WebGPU lost its
// GPU process.
@group(1) @binding(0) var<uniform> draw_words: array<vec4f, 1>;

fn draw_translation() -> vec3f { return draw_words[0].xyz; }
fn draw__p() -> f32 { return draw_words[0].w; }

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
