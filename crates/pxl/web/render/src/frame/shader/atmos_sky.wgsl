// The sky-view table and its lookup: the sky along a direction from the
// camera's altitude, unexposed, for a sun of illuminance 1. `sun` is toward
// the sun in the frame the direction is in, since only the angle between
// their azimuths reads the table.

@group(0) @binding(10) var sky_view_lut: texture_2d<f32>;

// A sky-view table along `dir`, lit by a light toward `toward` in the same
// frame: the sun's table or the moon's (atmos_view.wgsl), one lookup.
fn table_view(lut: texture_2d<f32>, dir: vec3f, toward: vec3f) -> vec3f {
  let h_dir = length(dir.xy);
  let h_light = length(toward.xy);
  var light_cos = 1.0;
  if (h_dir > 1e-4 && h_light > 1e-4) {
    light_cos = dot(dir.xy, toward.xy) / (h_dir * h_light);
  }
  return textureSampleLevel(lut, atmos_sampler, sky_view_uv(camera_r(), dir.z, light_cos), 0.0).rgb;
}

fn sky_view(dir: vec3f, sun: vec3f) -> vec3f {
  return table_view(sky_view_lut, dir, sun);
}
