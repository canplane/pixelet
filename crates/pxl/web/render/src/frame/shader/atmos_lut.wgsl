// What the transmittance table answers: how much light survives from the
// top of the atmosphere to a point, along a ray, and so from the sun.

fn transmittance_to_top(r: f32, mu: f32) -> vec3f {
  return textureSampleLevel(transmittance_lut, atmos_sampler, transmittance_uv(r, mu), 0.0).rgb;
}

// The sun's light at a point, the ground's shadow included. The disc sets
// across its own width rather than at its center, as look/atmosphere.rs's
// sun_light has it, so the air's light does not switch off in one frame.
fn sun_transmittance(r: f32, mu_s: f32) -> vec3f {
  let horizon = -sqrt(max(1.0 - (atmos_bottom_radius() / r) * (atmos_bottom_radius() / r), 0.0));
  let above = clamp((mu_s - horizon) / (2.0 * atmos_sun_angular_radius()) + 0.5, 0.0, 1.0);
  return transmittance_to_top(r, max(mu_s, horizon + 1e-4)) * above;
}
