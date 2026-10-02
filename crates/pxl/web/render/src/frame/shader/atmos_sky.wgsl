// Sky-view table lookup, unexposed, for a light of illuminance 1.

@group(0) @binding(10) var sky_view_lut: texture_2d<f32>;

fn table_at(lut: texture_2d<f32>, mu: f32, light_cos: f32) -> vec3f {
  return textureSampleLevel(lut, atmos_sampler, sky_view_uv(camera_r(), mu, light_cos), 0.0).rgb;
}

fn table_view(lut: texture_2d<f32>, dir: vec3f, toward: vec3f) -> vec3f {
  let up = atmos_up();
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
