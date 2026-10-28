// Bruneton's irradiance table, once per device: the sky's light on an up-facing
// face by altitude and a light's height, per unit of it, read at the surface's
// own height. Alpha: the sky's mean luminance over the hemisphere, white balanced.

@fragment
fn fs(v: TableOut) -> @location(0) vec4f {
  let uv = v.clip.xy / IRRADIANCE_SIZE;
  let mu_s = unit_of(uv.x, IRRADIANCE_SIZE.x) * 2.0 - 1.0;
  let r = atmos_bottom_radius() + unit_of(uv.y, IRRADIANCE_SIZE.y) * (atmos_top_radius() - atmos_bottom_radius());
  let sun = view_light(mu_s);
  // Rings even in sin² of the zenith: the sum is pi times their mean, and each
  // over its cosine, half the mean is the hemisphere's.
  var sky = vec3f(0.0);
  var seen = vec3f(0.0);
  for (var i = 0; i < ATMOS_IRRADIANCE_RINGS; i++) {
    let mu = sqrt(1.0 - (f32(i) + 0.5) / f32(ATMOS_IRRADIANCE_RINGS));
    let s = sqrt(max(1.0 - mu * mu, 0.0));
    for (var j = 0; j < ATMOS_IRRADIANCE_BEARINGS; j++) {
      let phi = TAU * (f32(j) + 0.5) / f32(ATMOS_IRRADIANCE_BEARINGS);
      let l = sky_radiance(r, vec3f(s * cos(phi), s * sin(phi), mu), sun, ATMOS_IRRADIANCE_STEPS);
      sky += l;
      seen += l / mu;
    }
  }
  let n = f32(ATMOS_IRRADIANCE_RINGS * ATMOS_IRRADIANCE_BEARINGS);
  return vec4f(sky * PI / n, dot(seen * 0.5 / n * atmos_white(), LUMA));
}
