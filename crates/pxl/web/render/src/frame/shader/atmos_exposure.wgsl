// The eye's adaptation: the exposure the frame shows the air at, into a
// texel of its own. The horizontal illuminance now — the sky's, the
// sky-view table over the upper hemisphere, and the sun's through the air —
// against a sun overhead at sea level, which the white balance makes
// exactly 1: the exposure rises by a power of how far the light has fallen,
// up to the model's limit, and not below where it stands at full sun. Drawn
// after the sky-view table, whenever that is, with the zenith's sky beside
// it in the texel's other three channels.

const EXPOSURE_RINGS: i32 = 8;
const EXPOSURE_BEARINGS: i32 = 16;

@fragment
fn fs(v: TableOut) -> @location(0) vec4f {
  let mu_s = clamp(atmos.sun_dir.z, -1.0, 1.0);
  let sun = vec3f(sqrt(max(1.0 - mu_s * mu_s, 0.0)), 0.0, mu_s);
  // Cosine-weighted: rings evenly spaced in sin² of the zenith angle, so
  // each sample stands for an equal share of the irradiance and the sum is
  // pi times their mean.
  var sky = vec3f(0.0);
  for (var i = 0; i < EXPOSURE_RINGS; i++) {
    let mu = sqrt(1.0 - (f32(i) + 0.5) / f32(EXPOSURE_RINGS));
    let s = sqrt(max(1.0 - mu * mu, 0.0));
    for (var j = 0; j < EXPOSURE_BEARINGS; j++) {
      let phi = TAU * (f32(j) + 0.5) / f32(EXPOSURE_BEARINGS);
      sky += sky_view(vec3f(s * cos(phi), s * sin(phi), mu), sun);
    }
  }
  sky *= PI / f32(EXPOSURE_RINGS * EXPOSURE_BEARINGS);
  let direct = sun_transmittance(camera_r(), mu_s) * max(mu_s, 0.0);
  let light = dot((sky + direct) * atmos.white, LUMA);
  let adapted = clamp(pow(1.0 / max(light, 1e-6), atmos.adaptation), 1.0, atmos.max_adaptation);
  // Beside it, the sky straight up, unexposed, which the clouds are lit by
  // from above (atmos_view.wgsl's zenith_sky): one texel read a fragment
  // rather than a table's.
  return vec4f(atmos.exposure * adapted, sky_view(vec3f(0.0, 0.0, 1.0), sun));
}
