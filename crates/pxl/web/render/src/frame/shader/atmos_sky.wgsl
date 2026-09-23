// The sky-view table and its lookup: the sky along a direction from the
// camera's altitude, unexposed, for a sun of illuminance 1. Only a ray's
// height and the angle between its bearing and the light's read the table.

@group(0) @binding(10) var sky_view_lut: texture_2d<f32>;

// A sky-view table in its own frame — the zenith up, the light's azimuth
// along x — at a ray `mu` from the zenith whose azimuth is `light_cos` from
// the light's: the sun's table or the moon's (atmos_view.wgsl).
fn table_at(lut: texture_2d<f32>, mu: f32, light_cos: f32) -> vec3f {
  return textureSampleLevel(lut, atmos_sampler, sky_view_uv(camera_r(), mu, light_cos), 0.0).rgb;
}

// The same along `dir`, lit by a light toward `toward`, both in the root
// frame: their heights from the zenith at the eye, and the angle between
// their bearings round it.
fn table_view(lut: texture_2d<f32>, dir: vec3f, toward: vec3f) -> vec3f {
  let up = atmos.up;
  let mu = dot(dir, up);
  let dir_h = dir - mu * up;
  let light_h = toward - dot(toward, up) * up;
  let h_dir = length(dir_h);
  let h_light = length(light_h);
  var light_cos = 1.0;
  if (h_dir > 1e-4 && h_light > 1e-4) {
    light_cos = dot(dir_h, light_h) / (h_dir * h_light);
  }
  return table_at(lut, mu, light_cos);
}

fn sky_view(dir: vec3f, sun: vec3f) -> vec3f {
  return table_view(sky_view_lut, dir, sun);
}
