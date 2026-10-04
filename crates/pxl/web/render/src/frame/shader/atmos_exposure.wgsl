// Eye adaptation, measured on the land under the eye wherever the eye is: an
// orbit's own sky is black, and adapting to it blew the lit Earth out. Texel 0:
// exposure from horizontal illuminance against a sun overhead at sea level (1
// after white balance); texel 1: the sky's irradiance on the land, sun and moon.

@fragment
fn fs(v: TableOut) -> @location(0) vec4f {
  let r = land_radius();
  let mu_s = clamp(dot(atmos_sun_dir(), frame_up()), -1.0, 1.0);
  let sky = sky_irradiance(r, mu_s);
  if (v.clip.x >= 1.0) {
    let moon = sky_irradiance(r, dot(atmos_moon_dir(), frame_up())) * atmos_moon_light();
    return vec4f(sky + moon, 1.0);
  }
  let direct = sun_transmittance(r, mu_s) * max(mu_s, 0.0);
  let light = dot((sky + direct) * atmos_white(), LUMA);
  let adapted = clamp(pow(1.0 / max(light, 1e-6), atmos_adaptation()), 1.0, atmos_max_adaptation());
  return vec4f(atmos_exposure() * adapted, 0.0, 0.0, 1.0);
}
