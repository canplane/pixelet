// pxl-render/frame/shader/camera_grid.wgsl

// The camera grid fixed to the screen: the sphere about the eye cut into cells
// of one solid angle, a cell's angle squared (`frame_cell`, pxl-kernel
// domain/view/lens.rs `cell_angle`, τ / `SPLIT`), in rings a cell apart about the
// optical axis, each ring cut into as many as fit, so a cell here holds the
// area the eye decides a block's zoom by (pxl-spec-core grid.rs).

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
  let x = dot(dir, normalize(m[0]));
  let y = dot(dir, normalize(m[1]));
  let a = frame_cell();
  // acos would lose its bits at the axis.
  let r = atan2(length(vec2f(x, y)), dot(dir, normalize(m[2]))) / a;
  // Ring k spans [(k − ½)a, (k + ½)a]; the one on the axis is a single cell.
  let ring = round(r);
  let n = max(1.0, round(TAU * sin(ring * a) / a));
  let phi = atan2(y, x);
  // Measured on both sides of the seam at ±π, so it draws no line.
  let dphi = min(fwidth(phi), fwidth(fract(phi / TAU) * TAU));
  let sectors = edge(phi * n / TAU, n * dphi / TAU) * step(1.5, n);
  let line = max(edge(r - 0.5, fwidth(r)), sectors);
  return vec4f(to_srgb(COLOR.rgb), COLOR.a * line);
}
