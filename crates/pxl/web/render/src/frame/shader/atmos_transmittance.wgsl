// The transmittance table: from each (r, mu) a texel stands for, the light
// left after the air between there and the top, marched and summed. Drawn
// once for the device; the atmosphere does not change.

// Held as vec4 words and read back by `atmos()`: what the host and a shader
// share has no room inside it for a driver to lay out its own way
// (issue/20260927-a-matrix-the-driver-reads-its-own-way.md).
@group(0) @binding(8) var<uniform> atmos_words: array<vec4f, 14>;

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
    let h = sqrt(t * t + 2.0 * r * mu * t + r * r) - atmos().bottom_radius;
    depth += medium_at(h).extinction * dt;
  }
  return vec4f(exp(-depth), 1.0);
}
