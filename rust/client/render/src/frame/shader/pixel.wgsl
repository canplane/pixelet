// pxl-render/frame/shader/pixel.wgsl

// A drawn pixel's tangents across and up at `at`, its middle or a fragment's
// position, of the `size` drawn, row 0 at the top, the screen's half-angle
// tangents `tan`: pxl-shader cast.rs `pixel_tan`'s mirror, the one form the
// screen pass's rays (screen_ray.wgsl `aim`) and the raster's (raster.wgsl
// `raster_tan`) are aimed by, so a pixel's ray is the cast's.
fn pixel_tan(at: vec2f, size: vec2f, tan: vec2f) -> vec2f {
  let ndc = vec2f(at.x / size.x * 2.0 - 1.0, 1.0 - at.y / size.y * 2.0);
  return ndc * tan;
}
