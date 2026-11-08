// The optical depth from radius `r` along `mu` to the top of the air. The table
// holds optical depth rather than transmittance, so a stretch's depth is the
// difference of two reads: a ratio of two transmittances near zero is not exact.
fn depth_to_top(r: f32, mu: f32) -> vec3f {
  return textureSampleLevel(transmittance_lut, atmos_sampler, transmittance_uv(r, mu), 0.0).rgb;
}

fn transmittance_to_top(r: f32, mu: f32) -> vec3f {
  return exp(-depth_to_top(r, mu));
}

// The white that shows a sun overhead at sea level as white: the inverse of
// what the table lets through there, at its own texel (mu 1, the bottom row).
// The exposure pass writes it beside the eye's stops, where every pass after it
// reads it (atmos_eye.wgsl).
fn table_white() -> vec3f {
  return 1.0 / transmittance_to_top(atmos_bottom_radius(), 1.0);
}

// What the air lets through of the sun at radius `r` and cosine `mu_s`. The
// sun's disc sets across its own width, so the light the air passes does not
// switch off within one frame.
fn sun_transmittance(r: f32, mu_s: f32) -> vec3f {
  let horizon = -sqrt(max(1.0 - (atmos_bottom_radius() / r) * (atmos_bottom_radius() / r), 0.0));
  let above = clamp((mu_s - horizon) / (2.0 * atmos_sun_angular_radius()) + 0.5, 0.0, 1.0);
  // Once the disc has set whole, no light passes and the table is not read.
  if (above <= 0.0) {
    return vec3f(0.0);
  }
  return transmittance_to_top(r, max(mu_s, horizon + 1e-4)) * above;
}

// The optical depth from a ray's lowest point (`ray_low`) to `t` km along it,
// for a ray from radius `r` at cosine `mu` that stops short of the ground. It
// is negative before the lowest point. Every lookup faces up and away from that
// point, so a ray past the ground's horizon needs no case of its own.
fn depth_along(r: f32, mu: f32, low: vec3f, t: f32) -> vec3f {
  let out = r * mu + t;
  let rt = max(radius_along(r, mu, t), atmos_bottom_radius());
  return sign(out) * (low - depth_to_top(rt, min(abs(out) / rt, 1.0)));
}

// The optical depth to the top from the ray's lowest point. It is needed only
// where the stretch to `end` km passes that point; elsewhere it cancels from
// every difference, and zero stands for it.
fn ray_low(r: f32, mu: f32, end: f32) -> vec3f {
  if (-r * mu <= 0.0 || -r * mu >= end) {
    return vec3f(0.0);
  }
  return depth_to_top(max(r * sqrt(max(1.0 - mu * mu, 0.0)), atmos_bottom_radius()), 0.0);
}

// A ray stops where it meets the ground, as no air lies past it.
fn short_of_ground(r: f32, mu: f32, t: f32) -> f32 {
  return select(t, min(t, to_ground(r, mu)), meets_ground(r, mu));
}

// (1 - exp(-x)) / x, without the cancellation where x is small.
fn extinction_weight(x: vec3f) -> vec3f {
  return select((vec3f(1.0) - exp(-x)) / max(x, vec3f(1e-8)), vec3f(1.0) - x * 0.5 + x * x / 6.0, x < vec3f(0.001));
}
