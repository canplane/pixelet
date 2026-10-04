// Transmittance table, drawn once per device.

// Read field by field from vec4 words, never as a struct: a struct here lost
// Safari's WebGPU its GPU process.
@group(0) @binding(8) var<uniform> atmos_words: array<vec4f, 16>;

const TRANSMITTANCE_STEPS: i32 = 40;

@fragment
fn fs(v: TableOut) -> @location(0) vec4f {
  let rm = transmittance_r_mu(v.clip.xy / TRANSMITTANCE_SIZE);
  let r = rm.x;
  let mu = rm.y;
  let d = to_top(r, mu);
  let dt = d / f32(TRANSMITTANCE_STEPS);
  var depth = vec3f(0.0);
  for (var i = 0; i < TRANSMITTANCE_STEPS; i++) {
    let t = (f32(i) + 0.5) * dt;
    let h = sqrt(t * t + 2.0 * r * mu * t + r * r) - atmos_bottom_radius();
    depth += medium_at(h).extinction * dt;
  }
  return vec4f(exp(-depth), 1.0);
}
