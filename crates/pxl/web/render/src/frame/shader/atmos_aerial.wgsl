// Hillaire's aerial perspective, every frame: the light the air gathers along
// each screen ray to each slice, unexposed. A slice is a share of its ray's
// path, squared toward the eye, so the last is the sky-view table's.

// Unreal's two steps a slice (r.SkyAtmosphere.AerialPerspectiveLUT.SampleCountMaxPerSlice).
const AERIAL_STEPS_PER_SLICE: i32 = 2;

@fragment
fn fs(v: TableOut) -> @location(0) vec4f {
  let slice = floor(v.clip.x / AERIAL_SIZE.x);
  let at = vec2f(v.clip.x - slice * AERIAL_SIZE.x, v.clip.y) / AERIAL_SIZE.xy;
  let ndc = vec2f(unit_of(at.x, AERIAL_SIZE.x), unit_of(at.y, AERIAL_SIZE.y)) * 2.0 - 1.0;
  let ray = normalize(sky_ray() * vec3f(ndc, 1.0));
  // Into the march's frame, up +z.
  let up = frame_up();
  let east = normalize(select(cross(up, vec3f(0.0, 1.0, 0.0)), cross(up, vec3f(1.0, 0.0, 0.0)), abs(up.y) > 0.9));
  let north = cross(up, east);
  let local = vec3f(dot(ray, east), dot(ray, north), dot(ray, up));
  let r = camera_r();
  let reach = air_path(r, local.z) * pow((slice + 1.0) / AERIAL_SIZE.z, 2.0);
  let steps = AERIAL_STEPS_PER_SLICE * (i32(slice) + 1);
  let sun = atmos_sun_dir();
  var light = sky_radiance(r, local, vec3f(dot(sun, east), dot(sun, north), dot(sun, up)), reach, steps);
  if (atmos_moon_light() > 0.0) {
    let moon = atmos_moon_dir();
    light += sky_radiance(r, local, vec3f(dot(moon, east), dot(moon, north), dot(moon, up)), reach, steps) * atmos_moon_light();
  }
  return vec4f(light * atmos_white(), 1.0);
}
