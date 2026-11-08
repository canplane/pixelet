// pxl-render/frame/shader/screen_mask.wgsl

// The gate a frame not drawn whole is drawn through (gpu/renderer.rs `frame`):
// each claimed tile (screen_tiles.wgsl) set back to what a cleared pass starts
// from, no color and the far plane's depth, reversed, and its stencil raised,
// which every draw of the world's pass after it tests.

@fragment
fn fs(v: VertexOut) -> @location(0) vec4f {
  return vec4f(0.0);
}
