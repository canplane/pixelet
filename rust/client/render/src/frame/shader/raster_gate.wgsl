// pxl-render/frame/shader/raster_gate.wgsl

// The gate the samples' raster is drawn through in a frame not drawn whole
// (gpu/ray/raster.rs `draw`), its depth cleared to the nearest: each claimed
// tile (screen_tiles.wgsl) set back to what a cleared pass starts from, no
// sample and the far plane's depth, reversed, so the boxes pass the depth
// test there alone.

@fragment
fn fs(v: VertexOut) -> @location(0) vec4u {
  return vec4u(0u);
}
