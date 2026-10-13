// pxl-render/frame/shader/camera_grid.wgsl

// The camera grid fixed to the screen, the one the rays were cast through
// (`pxl-kernel view/sight.rs` `Lens`): from one edge of the view to the
// other, a cell at each step of angle across and up (`frame_grid`).

const COLOR = vec4f(1.0, 1.0, 1.0, 0.55);

struct VertexOut {
  @builtin(position) clip: vec4f,
  @location(0) ndc: vec2f,
};

@vertex
fn vs(@builtin(vertex_index) i: u32) -> VertexOut {
  let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u)) * 2.0 - 1.0;
  var o: VertexOut;
  o.clip = vec4f(p, 0.0, 1.0);
  o.ndc = p;
  return o;
}

// How much of a pixel `v` crossing an integer covers, `w` its step a pixel.
fn edge(v: f32, w: f32) -> f32 {
  return 1.0 - smoothstep(0.0, max(w, 1e-6), abs(v - round(v)));
}

@fragment
fn fs(v: VertexOut) -> @location(0) vec4f {
  let m = sky_ray();
  let dir = m * vec3f(v.ndc, 1.0);
  let forward = length(m[2]);
  let z = dot(dir, m[2] / forward);
  // Each axis's angle from the view's edge, counted in cells.
  let angle = vec2f(atan2(dot(dir, normalize(m[0])), z), atan2(dot(dir, normalize(m[1])), z));
  let half = vec2f(atan2(length(m[0]), forward), atan2(length(m[1]), forward));
  let u = (angle + half) / max(frame_grid(), vec2f(1e-9));
  let line = max(edge(u.x, fwidth(u.x)), edge(u.y, fwidth(u.y))) * step(1e-9, frame_grid().x);
  return vec4f(to_srgb(COLOR.rgb), COLOR.a * line);
}
