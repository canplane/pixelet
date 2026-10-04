// Hillaire's sky-view table, drawn once for the sun and once for the moon,
// each relative to its light's azimuth.

@group(0) @binding(14) var<uniform> light: vec4f;

const SKY_VIEW_STEPS: i32 = 30;

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

  let toward = select(atmos_sun_dir(), atmos_moon_dir(), light.x > 0.5);
  let mu_s = clamp(dot(toward, frame_up()), -1.0, 1.0);
  let sun = vec3f(sqrt(max(1.0 - mu_s * mu_s, 0.0)), 0.0, mu_s);

  return vec4f(sky_radiance(r, dir, sun, air_path(r, dir.z), SKY_VIEW_STEPS), 1.0);
}
