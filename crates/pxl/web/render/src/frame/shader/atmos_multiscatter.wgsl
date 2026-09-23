// Hillaire's multiple scattering table: for a point at each altitude under
// a sun at each height, what light scattered twice or more adds, as a
// factor on the local scattering. The second order is marched over a
// sphere of directions with an isotropic phase — the orders past the
// first have lost the phase's shape anyway — and the orders past it are
// that one's geometric series, 1 / (1 - f), `f` the fraction of light a
// point sends back into itself. Drawn once, after the transmittance table.

const MULTISCATTER_DIRS: i32 = 8;
const MULTISCATTER_STEPS: i32 = 20;

@fragment
fn fs(v: TableOut) -> @location(0) vec4f {
  let uv = v.clip.xy / MULTISCATTER_SIZE;
  let mu_s = unit_of(uv.x, MULTISCATTER_SIZE.x) * 2.0 - 1.0;
  let r = atmos.bottom_radius + unit_of(uv.y, MULTISCATTER_SIZE.y) * (atmos.top_radius - atmos.bottom_radius);
  let sun = vec3f(sqrt(max(1.0 - mu_s * mu_s, 0.0)), 0.0, mu_s);
  let origin = vec3f(0.0, 0.0, r);

  var second = vec3f(0.0);
  var back = vec3f(0.0);
  // A regular grid over the sphere, equal in area: cos(theta) evenly
  // spaced, the azimuth evenly spaced.
  for (var a = 0; a < MULTISCATTER_DIRS; a++) {
    for (var b = 0; b < MULTISCATTER_DIRS; b++) {
      let mu = 1.0 - 2.0 * (f32(a) + 0.5) / f32(MULTISCATTER_DIRS);
      let phi = TAU * (f32(b) + 0.5) / f32(MULTISCATTER_DIRS);
      let s = sqrt(max(1.0 - mu * mu, 0.0));
      let dir = vec3f(s * cos(phi), s * sin(phi), mu);
      let ground = meets_ground(r, mu);
      let d = select(to_top(r, mu), to_ground(r, mu), ground);
      let dt = d / f32(MULTISCATTER_STEPS);
      var through = vec3f(1.0);
      for (var i = 0; i < MULTISCATTER_STEPS; i++) {
        let p = origin + dir * ((f32(i) + 0.5) * dt);
        let pr = length(p);
        let m = medium_at(pr - atmos.bottom_radius);
        let step = exp(-m.extinction * dt);
        // The analytic integral over the step, of a source constant across
        // it: (S - S * T) / extinction.
        let fill = (vec3f(1.0) - step) / max(m.extinction, vec3f(1e-7));
        let lit = sun_transmittance(pr, dot(p / pr, sun));
        second += through * m.scattering * INV_4PI * lit * fill;
        back += through * m.scattering * fill;
        through *= step;
      }
      if (ground) {
        let p = origin + dir * d;
        let n = normalize(p);
        let lit = sun_transmittance(length(p), dot(n, sun));
        second += through * lit * max(dot(n, sun), 0.0) * atmos.ground_albedo / PI;
      }
    }
  }
  let n = f32(MULTISCATTER_DIRS * MULTISCATTER_DIRS);
  // The sphere's average, which is the integral against an isotropic
  // phase: the radiance scattered once more, and the fraction returned.
  let l2 = second / n;
  let f = back / n;
  return vec4f(l2 / (vec3f(1.0) - f), 1.0);
}
