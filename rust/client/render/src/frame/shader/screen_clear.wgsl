// pxl-render/frame/shader/screen_clear.wgsl

// The region a frame draws again, under the pass's scissor, set back to
// what a cleared pass starts from: no color, and the far plane's depth,
// reversed (gpu/surface.rs `Region`).

struct VertexOut {
  @builtin(position) clip: vec4f,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32) -> VertexOut {
  let p = array<vec2f, 3>(vec2f(-1.0, -1.0), vec2f(3.0, -1.0), vec2f(-1.0, 3.0));
  var o: VertexOut;
  o.clip = vec4f(p[vi], 0.0, 1.0);
  return o;
}

@fragment
fn fs(v: VertexOut) -> @location(0) vec4f {
  return vec4f(0.0);
}
