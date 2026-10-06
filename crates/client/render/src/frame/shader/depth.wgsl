// Reversed Z in f32, near 1 and far 0: float precision and perspective depth
// cancel, so far coplanar faces stop fighting. Maps back to GL's clip z.
fn depth_to_gl_z(d: f32) -> f32 {
  return 1.0 - 2.0 * d;
}
