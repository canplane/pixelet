// The eye's adaptation: the exposure the frame shows the air at, into a
// texel of its own. The horizontal illuminance now — the sky's, the
// sky-view table over the upper hemisphere, and the sun's through the air —
// against a sun overhead at sea level, which the white balance makes
// exactly 1: the exposure rises by a power of how far the light has fallen,
// up to the model's limit, and not below where it stands at full sun. Drawn
// after the sky-view table, whenever that is, into two texels: the exposure
// with the zenith's sky beside it in the other three channels, and the sky's
// own irradiance on a face turned up, what the terrain's fill is lit by.

const EXPOSURE_RINGS: i32 = 8;
const EXPOSURE_BEARINGS: i32 = 16;

@fragment
fn fs(v: TableOut) -> @location(0) vec4f {
  // In the table's own frame: the sun's height from the zenith at the eye,
  // and each ray's bearing from the sun's.
  let mu_s = clamp(dot(atmos().sun_dir, atmos().up), -1.0, 1.0);
  // Cosine-weighted: rings evenly spaced in sin² of the zenith angle, so
  // each sample stands for an equal share of the irradiance and the sum is
  // pi times their mean.
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
  let light = dot((sky + direct) * atmos().white, LUMA);
  let adapted = clamp(pow(1.0 / max(light, 1e-6), atmos().adaptation), 1.0, atmos().max_adaptation);
  // The second texel: the sky's irradiance alone, unexposed
  // (atmos_view.wgsl's sky_fill).
  if (v.clip.x >= 1.0) {
    return vec4f(sky, 1.0);
  }
  // Beside the exposure, the sky straight up, unexposed, which the clouds
  // are lit by from above (atmos_view.wgsl's zenith_sky): one texel read a
  // fragment rather than a table's.
  return vec4f(atmos().exposure * adapted, table_at(sky_view_lut, 1.0, 1.0));
}
