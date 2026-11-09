// pxl-render/frame/shader/screen_mask.wgsl

// The gate a frame not drawn whole is drawn through (renderer.rs `frame`). It sets each
// claimed tile (screen_tile.wgsl) back to what a cleared pass starts from: no color, and the
// far plane's depth, which is 0 as the depth is reversed. It also raises the tile's stencil,
// which every later draw of the world's pass tests, so those draws touch only the claimed
// tiles.

@fragment
fn fs(v: VertexOut) -> @location(0) vec4f {
  return vec4f(0.0);
}
