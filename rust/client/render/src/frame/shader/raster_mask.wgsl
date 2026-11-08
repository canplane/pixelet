// pxl-render/frame/shader/raster_mask.wgsl

// The gate the samples' raster is drawn through in a frame not drawn whole
// (gpu/ray/raster.rs `draw`). The raster's depth is cleared to the nearest,
// and the gate sets each claimed tile (screen_tiles.wgsl) back to what a
// cleared pass starts from: no sample, and the far plane's depth, which is 0
// as the depth is reversed. So the boxes pass the depth test in the claimed
// tiles alone.

@fragment
fn fs(v: VertexOut) -> @location(0) vec4u {
  return vec4u(0u);
}
