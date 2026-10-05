// Sky-view table lookup, unexposed, for a light of illuminance 1.

@group(0) @binding(10) var sky_view_lut: texture_2d<f32>;

// The cosine between `dir`'s azimuth and the light's, as the tables measure it.
fn light_cos_of(dir: vec3f, toward: vec3f) -> f32 {
  let up = frame_up();
  let dir_h = dir - dot(dir, up) * up;
  let light_h = toward - dot(toward, up) * up;
  let h_dir = length(dir_h);
  let h_light = length(light_h);
  if (h_dir > 1e-4 && h_light > 1e-4) {
    return dot(dir_h, light_h) / (h_dir * h_light);
  }
  return 1.0;
}

fn table_view(lut: texture_2d<f32>, dir: vec3f, toward: vec3f) -> vec3f {
  return textureSampleLevel(lut, atmos_sampler, sky_view_uv(camera_r(), dot(dir, frame_up()), light_cos_of(dir, toward)), 0.0).rgb;
}

fn sky_view(dir: vec3f, sun: vec3f) -> vec3f {
  return table_view(sky_view_lut, dir, sun);
}
