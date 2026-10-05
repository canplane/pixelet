// What a ray from `r` along `dir`, up +z, gathers over its first `reach` km
// from a light of one toward `toward`: single scattering marched, Hillaire's
// table for the orders past it.

@group(0) @binding(12) var multiscatter_lut: texture_2d<f32>;

fn multiscatter(r: f32, mu_s: f32) -> vec3f {
  return textureSampleLevel(multiscatter_lut, atmos_sampler, multiscatter_uv(r, mu_s), 0.0).rgb;
}

fn sky_radiance(r: f32, dir: vec3f, toward: vec3f, reach: f32, steps: i32) -> vec3f {
  let mu = dir.z;
  let c = dot(dir, toward);
  let phase_r = rayleigh_phase(c);
  let phase_m = mie_phase(c);
  let d = min(air_path(r, mu), reach);
  // From above the air, where the ray enters it.
  var start = 0.0;
  if (r > atmos_top_radius()) {
    let disc = r * r * (mu * mu - 1.0) + atmos_top_radius() * atmos_top_radius();
    if (disc < 0.0 || mu > 0.0) {
      return vec3f(0.0);
    }
    start = -r * mu - sqrt(disc);
  }
  let origin = vec3f(0.0, 0.0, r) + dir * start;
  let span = d - start;
  if (span <= 0.0) {
    return vec3f(0.0);
  }
  var through = vec3f(1.0);
  var light = vec3f(0.0);
  // Squared toward the eye, where the air is densest.
  for (var i = 0; i < steps; i++) {
    let t0 = f32(i) / f32(steps);
    let t1 = f32(i + 1) / f32(steps);
    let ta = t0 * t0 * span;
    let tb = t1 * t1 * span;
    let t = ta + (tb - ta) * 0.3;
    let dt = tb - ta;
    let p = origin + dir * t;
    let pr = length(p);
    let up = p / pr;
    let local_mu_s = dot(up, toward);
    let m = medium_at(pr - atmos_bottom_radius());
    let lit = sun_transmittance(pr, local_mu_s);
    let source = lit * (m.rayleigh * phase_r + m.mie * phase_m) + multiscatter(pr, local_mu_s) * m.scattering;
    let step = exp(-m.extinction * dt);
    light += through * source * (vec3f(1.0) - step) / max(m.extinction, vec3f(1e-7));
    through *= step;
  }
  return light;
}
