// pxl-render/frame/shader/debug.wgsl

// Debug gizmos as segments, hidden behind the gizmos' own faces and nothing
// else; the faces draw only their depth (`vs_depth`). A line primitive is one pixel wide on every device, so each segment
// is a quad its width across on the screen.

const WIDTH_PX: f32 = 3.0;

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
  @location(0) color: vec4f,
};

// Two triangles: which end, and which side of the segment.
const ENDS = array<u32, 6>(0u, 0u, 1u, 1u, 0u, 1u);
const SIDES = array<f32, 6>(-1.0, 1.0, -1.0, -1.0, 1.0, 1.0);

@vertex
fn vs(@builtin(vertex_index) vi: u32, @location(0) a: vec3f, @location(1) color: vec4f, @location(2) b: vec3f) -> VertexOut {
  let ca = frame_view_proj() * vec4f(a + draw_translation(), 1.0);
  let cb = frame_view_proj() * vec4f(b + draw_translation(), 1.0);
  let viewport = frame_screen();
  let along = (cb.xy / max(cb.w, 1e-6) - ca.xy / max(ca.w, 1e-6)) * viewport;
  let dir = select(vec2f(1.0, 0.0), normalize(along), dot(along, along) > 1e-12);
  // Half the width each side, in clip units at w = 1.
  let across = vec2f(-dir.y, dir.x) * WIDTH_PX / viewport;
  let c = select(ca, cb, ENDS[vi] == 1u);
  var o: VertexOut;
  o.clip = vec4f(c.xy + across * SIDES[vi] * c.w, c.z, c.w);
  o.color = color;
  return o;
}

@vertex
fn vs_depth(@location(0) p: vec3f) -> @builtin(position) vec4f {
  return frame_view_proj() * vec4f(p + draw_translation(), 1.0);
}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  return vec4f(to_srgb(i.color.rgb), i.color.a);
}
