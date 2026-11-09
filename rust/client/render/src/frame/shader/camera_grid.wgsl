// pxl-render/camera_grid.wgsl

// The camera grid fixed to the screen, drawn over the canvas: the cells the rays are spaced by,
// squares of the screen's tangent plane a cell (`GRID_CELL`, pxl-engine grid.rs `CELL`) a side,
// counted from the screen's left and top (pxl-engine `beam::Grid::cell`). Its lines are
// `GRID_COLOR` (debug_look.json).

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

// How much of a pixel `v` crossing an integer covers, `w` its step across a pixel.
fn edge(v: f32, w: f32) -> f32 {
  return 1.0 - smoothstep(0.0, max(w, 1e-6), abs(v - round(v)));
}

@fragment
fn fs(v: VertexOut) -> @location(0) vec4f {
  let m = sky_ray();
  let tan = vec2f(length(m[0]), length(m[1]));
  let c = (vec2f(1.0) + vec2f(v.ndc.x, -v.ndc.y)) * tan / GRID_CELL;
  let w = fwidth(c);
  let line = max(edge(c.x, w.x), edge(c.y, w.y));
  return vec4f(to_srgb(GRID_COLOR.rgb), GRID_COLOR.a * line);
}
