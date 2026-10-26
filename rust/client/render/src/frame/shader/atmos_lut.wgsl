// The table holds optical depth, so a stretch's is a difference: a ratio of two
// transmittances near zero is not exact.
fn depth_to_top(r: f32, mu: f32) -> vec3f {
  return textureSampleLevel(transmittance_lut, atmos_sampler, transmittance_uv(r, mu), 0.0).rgb;
}

fn transmittance_to_top(r: f32, mu: f32) -> vec3f {
  return exp(-depth_to_top(r, mu));
}

// A sun overhead at sea level as white: the inverse of what the table leaves
// of it there, its own texel (mu 1, the bottom). The exposure pass writes it
// beside the eye's stops, where every pass after it reads it (atmos_eye.wgsl).
fn table_white() -> vec3f {
  return 1.0 / transmittance_to_top(atmos_bottom_radius(), 1.0);
}

// The disc sets across its own width, so the air's light does not switch off in one frame.
fn sun_transmittance(r: f32, mu_s: f32) -> vec3f {
  let horizon = -sqrt(max(1.0 - (atmos_bottom_radius() / r) * (atmos_bottom_radius() / r), 0.0));
  let above = clamp((mu_s - horizon) / (2.0 * atmos_sun_angular_radius()) + 0.5, 0.0, 1.0);
  return transmittance_to_top(r, max(mu_s, horizon + 1e-4)) * above;
}

// Optical depth from a ray's lowest point (`ray_low`) to `t` km along it, short
// of the ground, from radius `r` at cosine `mu`, negative before that point.
// Every lookup faces up and out of it, so a ray past the ground's horizon needs
// no case of its own.
fn depth_along(r: f32, mu: f32, low: vec3f, t: f32) -> vec3f {
  let out = r * mu + t;
  let rt = max(radius_along(r, mu, t), atmos_bottom_radius());
  return sign(out) * (low - depth_to_top(rt, min(abs(out) / rt, 1.0)));
}

// The depth at the lowest point, wanted only where the stretch to `end` km
// passes it: elsewhere it cancels from every difference.
fn ray_low(r: f32, mu: f32, end: f32) -> vec3f {
  if (-r * mu <= 0.0 || -r * mu >= end) {
    return vec3f(0.0);
  }
  return depth_to_top(max(r * sqrt(max(1.0 - mu * mu, 0.0)), atmos_bottom_radius()), 0.0);
}

// No air past the ground: a ray stops where it meets it.
fn short_of_ground(r: f32, mu: f32, t: f32) -> f32 {
  return select(t, min(t, to_ground(r, mu)), meets_ground(r, mu));
}

// (1 - exp(-x)) / x, without the cancellation where x is small.
fn extinction_weight(x: vec3f) -> vec3f {
  return select((vec3f(1.0) - exp(-x)) / max(x, vec3f(1e-8)), vec3f(1.0) - x * 0.5 + x * x / 6.0, x < vec3f(0.001));
}
