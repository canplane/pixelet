// pxl-render/frame/shader/decor_ball.wgsl

// The ball a decor instance stands within, about its middle, as wide as decor is tall and
// wide (pxl-content `generation::TALLEST`, `WIDEST`): what the light's marks see a drawn
// receiver by (light_mark_draws.wgsl) and a gated frame draws decor by (decor_gate.wgsl).

/// The ball's middle over `v`, where the instance stands.
fn decor_middle(v: vec3<f32>) -> vec3<f32> {
  return v + vec3<f32>(0.0, 0.0, f32(TALLEST) * 0.5);
}

/// The ball's radius.
fn decor_radius() -> f32 {
  return f32(TALLEST + WIDEST);
}
