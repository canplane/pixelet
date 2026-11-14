// pxl-render/frame/shader/atmos_eye.unit.wgsl

// The eye for a pass that runs before the exposure texel is written, or reads none:
// the exposure pass itself, the sky views and the irradiance table. Such a pass sees
// at an exposure of one and with the table's own white.

fn exposure() -> f32 {
  return 1.0;
}

fn atmos_white() -> vec3f {
  return table_white();
}
