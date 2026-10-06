// Hillaire's sky-view table, drawn once for the sun and once for the moon,
// each relative to its light's azimuth.

@group(0) @binding(14) var<uniform> light: vec4f;

const SKY_VIEW_STEPS: i32 = 30;

@fragment
fn fs(v: TableOut) -> @location(0) vec4f {
  let uv = v.clip.xy / SKY_VIEW_SIZE;
  let r = camera_r();
  let dir = view_dir(r, vec2f(unit_of(uv.x, SKY_VIEW_SIZE.x), unit_of(uv.y, SKY_VIEW_SIZE.y)));
  let sun = view_light(dot(select(atmos_sun_dir(), atmos_moon_dir(), light.x > 0.5), frame_up()));
  return vec4f(sky_radiance(r, dir, sun, SKY_VIEW_STEPS), 1.0);
}
