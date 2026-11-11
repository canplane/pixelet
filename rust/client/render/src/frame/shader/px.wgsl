// pxl-render/frame/shader/px.wgsl

// The tangents across and up of a drawn pixel's ray at `at`, which is the
// pixel's middle or a fragment's position. The target is `size` pixels with
// row 0 at the top, and `tan` holds the screen's half-angle tangents. The
// cast's rays (grid.wgsl `px_tan`), the screen pass's (screen_ray.wgsl `aim`)
// and the raster's (raster.wgsl `raster_tan`) are aimed by it alone, so a
// pixel's ray is the one the cast walked.
fn tan_at(at: vec2f, size: vec2f, tan: vec2f) -> vec2f {
  let ndc = vec2f(at.x / size.x * 2.0 - 1.0, 1.0 - at.y / size.y * 2.0);
  return ndc * tan;
}
