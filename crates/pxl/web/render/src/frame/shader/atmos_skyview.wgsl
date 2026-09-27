// The sky as seen from the camera's altitude (Hillaire's sky-view table):
// for each direction a texel stands for (atmos_medium.wgsl's sky_view_uv,
// inverted), the light the air scatters toward the eye between it and the
// ground or the top, marched with the light's at each step and the
// multiple scattering table's addition. Drawn twice, into two tables — lit
// by the sun and lit by the moon, `light`'s first word saying which, each
// relative to its own light's azimuth — and each only when its light's
// height or the camera's altitude has moved (gpu/atmosphere.rs).

@group(0) @binding(12) var multiscatter_lut: texture_2d<f32>;
@group(0) @binding(14) var<uniform> light: vec4f;

const SKY_VIEW_STEPS: i32 = 30;

fn multiscatter(r: f32, mu_s: f32) -> vec3f {
  return textureSampleLevel(multiscatter_lut, atmos_sampler, multiscatter_uv(r, mu_s), 0.0).rgb;
}

@fragment
fn fs(v: TableOut) -> @location(0) vec4f {
  let uv = v.clip.xy / SKY_VIEW_SIZE;
  let x = unit_of(uv.x, SKY_VIEW_SIZE.x);
  let y = unit_of(uv.y, SKY_VIEW_SIZE.y);
  let r = camera_r();

  let beta = horizon_dip(r);
  let zenith_horizon = PI - beta;
  var view_zenith: f32;
  if (y < 0.5) {
    let c = 1.0 - 2.0 * y;
    view_zenith = zenith_horizon * (1.0 - c * c);
  } else {
    let c = 2.0 * y - 1.0;
    view_zenith = zenith_horizon + beta * c * c;
  }
  let mu = cos(view_zenith);
  let light_cos = 1.0 - 2.0 * x * x;
  let s = sin(view_zenith);
  let dir = vec3f(s * light_cos, s * sqrt(max(1.0 - light_cos * light_cos, 0.0)), mu);

  let toward = select(atmos().sun_dir, atmos().moon_dir, light.x > 0.5);
  let mu_s = clamp(dot(toward, atmos().up), -1.0, 1.0);
  let sun = vec3f(sqrt(max(1.0 - mu_s * mu_s, 0.0)), 0.0, mu_s);
  let c = dot(dir, sun);
  let phase_r = rayleigh_phase(c);
  let phase_m = mie_phase(c);

  let d = select(to_top(r, mu), to_ground(r, mu), meets_ground(r, mu));
  // From above the atmosphere the march starts where the ray enters it,
  // and a ray that passes it by has no air at all.
  var start = 0.0;
  if (r > atmos().top_radius) {
    let disc = r * r * (mu * mu - 1.0) + atmos().top_radius * atmos().top_radius;
    if (disc < 0.0 || mu > 0.0) {
      return vec4f(0.0, 0.0, 0.0, 1.0);
    }
    start = -r * mu - sqrt(disc);
  }
  let origin = vec3f(0.0, 0.0, r) + dir * start;
  let span = d - start;
  var through = vec3f(1.0);
  var light = vec3f(0.0);
  // Steps squared toward the far end, a sample 0.3 of the way into each:
  // the air is densest, and the view changes most, close to the eye.
  for (var i = 0; i < SKY_VIEW_STEPS; i++) {
    let t0 = f32(i) / f32(SKY_VIEW_STEPS);
    let t1 = f32(i + 1) / f32(SKY_VIEW_STEPS);
    let ta = t0 * t0 * span;
    let tb = t1 * t1 * span;
    let t = ta + (tb - ta) * 0.3;
    let dt = tb - ta;
    let p = origin + dir * t;
    let pr = length(p);
    let up = p / pr;
    let local_mu_s = dot(up, sun);
    let m = medium_at(pr - atmos().bottom_radius);
    let lit = sun_transmittance(pr, local_mu_s);
    let source = lit * (m.rayleigh * phase_r + m.mie * phase_m) + multiscatter(pr, local_mu_s) * m.scattering;
    let step = exp(-m.extinction * dt);
    light += through * source * (vec3f(1.0) - step) / max(m.extinction, vec3f(1e-7));
    through *= step;
  }
  return vec4f(light, 1.0);
}
