// The eye at one, for the exposure pass, which measures before it.

fn exposure_stops() -> f32 {
  return 0.0;
}

fn exposure() -> f32 {
  return 1.0;
}

// The table's own white, for a pass before the exposure texel or with none.
fn atmos_white() -> vec3f {
  return table_white();
}
