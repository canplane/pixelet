// pxl-render/frame/shader/decor_ball.wgsl

// The ball that holds a decor instance, centered at its middle, with a radius as large as the
// tallest decor's height and the widest decor's width together (pxl-content
// `generation::TALLEST`, `WIDEST`). The light's marks see a drawn receiver by this ball
// (light_mark_draws.wgsl), and a gated frame draws decor by it (decor_cull.wgsl).

/// The ball's middle over `v`, where the instance stands.
fn decor_middle(v: vec3<f32>) -> vec3<f32> {
  return v + vec3<f32>(0.0, 0.0, f32(TALLEST) * 0.5);
}

fn decor_radius() -> f32 {
  return f32(TALLEST + WIDEST);
}
