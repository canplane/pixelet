// The eye's exposure, in stops, for the lights as the words say them
// (atmos_light.wgsl `eye_stops`), and beside it the table's white, which the
// passes after it read here (atmos_eye.wgsl).

@fragment
fn fs(v: TableOut) -> @location(0) vec4f {
  return vec4f(eye_stops(lights_now()), table_white());
}
