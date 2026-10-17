// The eye's exposure, in stops, for the lights as the words say them
// (atmos_light.wgsl `eye_stops`).

@fragment
fn fs(v: TableOut) -> @location(0) vec4f {
  return vec4f(eye_stops(lights_now()), 0.0, 0.0, 1.0);
}
