// pxl-render/frame/shader/screen_gate.wgsl

// The gate a frame not drawn whole is drawn through (gpu/renderer.rs `frame`):
// each claimed tile (screen_tiles.wgsl) set back to what a cleared pass starts
// from, no color and the far plane's depth, reversed, and its stencil raised,
// which every draw of the world's pass after it tests.

struct VertexOut {
  @builtin(position) clip: vec4f,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VertexOut {
  var o: VertexOut;
  o.clip = tile_clip(vi, ii);
  return o;
}

@fragment
fn fs(v: VertexOut) -> @location(0) vec4f {
  return vec4f(0.0);
}
