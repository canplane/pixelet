// The depth convention every pass shares: reversed Z in a 32-bit float,
// near at 1 and far at 0. A float's precision is densest near zero, and a
// perspective projection's depth is densest near the eye, so the two
// cancel and the far terrain's coplanar faces stop fighting — the fix every
// engine reached for once a float depth format was there to take it. GL's
// clip z is [-1, 1] with far at 1; the renderer's GL_TO_WGPU
// (gpu/renderer.rs) maps it to [1, 0], and this maps a depth value back.
fn depth_to_gl_z(d: f32) -> f32 {
  return 1.0 - 2.0 * d;
}
