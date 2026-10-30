// Hillaire's multiple scattering table: second order marched with an isotropic
// phase, higher orders as its geometric series 1 / (1 - f).

@fragment
fn fs(v: TableOut) -> @location(0) vec4f {
  let uv = v.clip.xy / MULTISCATTER_SIZE;
  let mu_s = unit_of(uv.x, MULTISCATTER_SIZE.x) * 2.0 - 1.0;
  let r = atmos_bottom_radius() + unit_of(uv.y, MULTISCATTER_SIZE.y) * (atmos_top_radius() - atmos_bottom_radius());
  let sun = vec3f(sqrt(max(1.0 - mu_s * mu_s, 0.0)), 0.0, mu_s);
  let origin = vec3f(0.0, 0.0, r);

  var second = vec3f(0.0);
  var back = vec3f(0.0);
  // Equal-area grid: cos(theta) and azimuth evenly spaced.
  for (var a = 0; a < ATMOS_MULTISCATTER_DIRS; a++) {
    for (var b = 0; b < ATMOS_MULTISCATTER_DIRS; b++) {
      let mu = 1.0 - 2.0 * (f32(a) + 0.5) / f32(ATMOS_MULTISCATTER_DIRS);
      let phi = TAU * (f32(b) + 0.5) / f32(ATMOS_MULTISCATTER_DIRS);
      let s = sqrt(max(1.0 - mu * mu, 0.0));
      let dir = vec3f(s * cos(phi), s * sin(phi), mu);
      let ground = meets_ground(r, mu);
      let d = air_path(r, mu);
      let dt = d / f32(ATMOS_MULTISCATTER_STEPS);
      var transmittance = vec3f(1.0);
      for (var i = 0; i < ATMOS_MULTISCATTER_STEPS; i++) {
        let p = origin + dir * ((f32(i) + 0.5) * dt);
        let pr = length(p);
        let m = medium_at(pr - atmos_bottom_radius());
        let step = exp(-m.extinction * dt);
        // Analytic integral of a constant source over the step.
        let fill = (vec3f(1.0) - step) / max(m.extinction, vec3f(1e-7));
        let lit = sun_transmittance(pr, dot(p / pr, sun));
        second += transmittance * m.scattering * INV_4PI * lit * fill;
        back += transmittance * m.scattering * fill;
        transmittance *= step;
      }
      if (ground) {
        let p = origin + dir * d;
        let n = normalize(p);
        let lit = sun_transmittance(length(p), dot(n, sun));
        second += transmittance * lit * max(dot(n, sun), 0.0) * atmos_ground_albedo() / PI;
      }
    }
  }
  let n = f32(ATMOS_MULTISCATTER_DIRS * ATMOS_MULTISCATTER_DIRS);
  let l2 = second / n;
  let f = back / n;
  return vec4f(l2 / (vec3f(1.0) - f), 1.0);
}
