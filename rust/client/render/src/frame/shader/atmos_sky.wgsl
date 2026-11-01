// Sky-view table lookup, unexposed, for a light of illuminance 1.

@group(0) @binding(10) var sky_view_lut: texture_2d<f32>;

// Where `dir` lies about light `k`, as its sky view and its share of the
// air's volume are laid (atmos_medium.wgsl `view_unit`, `laid`).
fn table_unit(dir: vec3f, k: u32) -> vec2f {
  let up = frame_up();
  let mu = dot(dir, up);
  let toward = light_dir(k);
  let dir_h = dir - mu * up;
  let light_h = toward - dot(toward, up) * up;
  let h_dir = length(dir_h);
  let h_light = length(light_h);
  var light_cos = 1.0;
  if (h_dir > 1e-4 && h_light > 1e-4) {
    light_cos = dot(dir_h, light_h) / (h_dir * h_light);
  }
  return view_unit(laid(k).x, mu, light_cos);
}

fn table_view(lut: texture_2d<f32>, dir: vec3f, k: u32) -> vec3f {
  let u = table_unit(dir, k);
  let uv = vec2f(coord_of(u.x, SKY_VIEW_SIZE.x), coord_of(u.y, SKY_VIEW_SIZE.y));
  return textureSampleLevel(lut, atmos_sampler, uv, 0.0).rgb;
}

fn sky_view(dir: vec3f) -> vec3f {
  return table_view(sky_view_lut, dir, 0u);
}
