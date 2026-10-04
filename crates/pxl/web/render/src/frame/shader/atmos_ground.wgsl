// The sky's light on an up-facing face at radius `r`, a light at cosine `mu_s`
// over it, per unit of that light (atmos_irradiance.wgsl).

@group(0) @binding(18) var irradiance_lut: texture_2d<f32>;

fn sky_irradiance(r: f32, mu_s: f32) -> vec3f {
  let x = coord_of(clamp(mu_s, -1.0, 1.0) * 0.5 + 0.5, IRRADIANCE_SIZE.x);
  let y = coord_of(clamp((r - atmos_bottom_radius()) / (atmos_top_radius() - atmos_bottom_radius()), 0.0, 1.0), IRRADIANCE_SIZE.y);
  return textureSampleLevel(irradiance_lut, atmos_sampler, vec2f(x, y), 0.0).rgb;
}
