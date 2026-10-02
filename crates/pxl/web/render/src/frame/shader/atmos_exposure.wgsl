// Eye adaptation: exposure from horizontal illuminance against a sun overhead
// at sea level (1 after white balance). Texel 0: exposure and zenith sky;
// texel 1: the sky's irradiance on an up-facing face.

const EXPOSURE_RINGS: i32 = 8;
const EXPOSURE_BEARINGS: i32 = 16;

@fragment
fn fs(v: TableOut) -> @location(0) vec4f {
  let mu_s = clamp(dot(atmos_sun_dir(), atmos_up()), -1.0, 1.0);
  // Cosine-weighted rings, evenly spaced in sin² of the zenith angle, so the sum
  // is pi times their mean.
  var sky = vec3f(0.0);
  for (var i = 0; i < EXPOSURE_RINGS; i++) {
    let mu = sqrt(1.0 - (f32(i) + 0.5) / f32(EXPOSURE_RINGS));
    for (var j = 0; j < EXPOSURE_BEARINGS; j++) {
      let phi = TAU * (f32(j) + 0.5) / f32(EXPOSURE_BEARINGS);
      sky += table_at(sky_view_lut, mu, cos(phi));
    }
  }
  sky *= PI / f32(EXPOSURE_RINGS * EXPOSURE_BEARINGS);
  let direct = sun_transmittance(camera_r(), mu_s) * max(mu_s, 0.0);
  let light = dot((sky + direct) * atmos_white(), LUMA);
  let adapted = clamp(pow(1.0 / max(light, 1e-6), atmos_adaptation()), 1.0, atmos_max_adaptation());
  if (v.clip.x >= 1.0) {
    return vec4f(sky, 1.0);
  }
  return vec4f(atmos_exposure() * adapted, table_at(sky_view_lut, 1.0, 1.0));
}
