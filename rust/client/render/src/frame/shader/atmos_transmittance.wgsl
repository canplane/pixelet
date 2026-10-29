// Transmittance table, drawn once per device, as optical depth to the top.

@fragment
fn fs(v: TableOut) -> @location(0) vec4f {
  let rm = transmittance_r_mu(v.clip.xy / TRANSMITTANCE_SIZE);
  let r = rm.x;
  let mu = rm.y;
  let d = to_top(r, mu);
  let dt = d / f32(ATMOS_TRANSMITTANCE_STEPS);
  var depth = vec3f(0.0);
  for (var i = 0; i < ATMOS_TRANSMITTANCE_STEPS; i++) {
    let t = (f32(i) + 0.5) * dt;
    depth += medium_at(height_along(r, mu, t, radius_along(r, mu, t))).extinction * dt;
  }
  return vec4f(depth, 1.0);
}
