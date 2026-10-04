// Bruneton's irradiance table: the sky's light on an up-facing face at each
// altitude for each height of a light, per unit of it, drawn once per device.
// What a surface is lit by is read at its own height, never the eye's. Alpha:
// the sky's mean luminance over the upper hemisphere, white balanced, what the
// eye's meter reads of it.

const IRRADIANCE_RINGS: i32 = 8;
const IRRADIANCE_BEARINGS: i32 = 16;
const IRRADIANCE_STEPS: i32 = 16;

@fragment
fn fs(v: TableOut) -> @location(0) vec4f {
  let uv = v.clip.xy / IRRADIANCE_SIZE;
  let mu_s = unit_of(uv.x, IRRADIANCE_SIZE.x) * 2.0 - 1.0;
  let r = atmos_bottom_radius() + unit_of(uv.y, IRRADIANCE_SIZE.y) * (atmos_top_radius() - atmos_bottom_radius());
  let sun = vec3f(sqrt(max(1.0 - mu_s * mu_s, 0.0)), 0.0, mu_s);
  // Cosine-weighted rings, evenly spaced in sin² of the zenith angle, so the sum
  // is pi times their mean; each over its cosine, half their mean is the mean
  // over the hemisphere.
  var sky = vec3f(0.0);
  var seen = vec3f(0.0);
  for (var i = 0; i < IRRADIANCE_RINGS; i++) {
    let mu = sqrt(1.0 - (f32(i) + 0.5) / f32(IRRADIANCE_RINGS));
    let s = sqrt(max(1.0 - mu * mu, 0.0));
    for (var j = 0; j < IRRADIANCE_BEARINGS; j++) {
      let phi = TAU * (f32(j) + 0.5) / f32(IRRADIANCE_BEARINGS);
      let l = sky_radiance(r, vec3f(s * cos(phi), s * sin(phi), mu), sun, IRRADIANCE_STEPS);
      sky += l;
      seen += l / mu;
    }
  }
  let n = f32(IRRADIANCE_RINGS * IRRADIANCE_BEARINGS);
  return vec4f(sky * PI / n, dot(seen * 0.5 / n * atmos_white(), LUMA));
}
