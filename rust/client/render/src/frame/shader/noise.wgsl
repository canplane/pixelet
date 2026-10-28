// Value noise over the cloud field's hash, `lattice_hash`, which build.rs
// writes from pxl-spec-core cloud.rs, where the cumulus draws its cells from
// it on the CPU.

// Wraps at `mask + 1`, the lattice's count across the world, so the noise wraps with the map.
fn lattice_noise(base: vec2u, p: vec2f, mask: u32, salt: u32) -> f32 {
  let whole = floor(p);
  let f = p - whole;
  let i = base + bitcast<vec2u>(vec2i(whole));
  let a = lattice_hash(i & vec2u(mask), salt);
  let b = lattice_hash((i + vec2u(1u, 0u)) & vec2u(mask), salt);
  let c = lattice_hash((i + vec2u(0u, 1u)) & vec2u(mask), salt);
  let d = lattice_hash((i + vec2u(1u, 1u)) & vec2u(mask), salt);
  let u = f * f * (3.0 - 2.0 * f);
  return mix(a, b, u.x) + (c - a) * u.y * (1.0 - u.x) + (d - b) * u.x * u.y;
}

// Octaves finer than half a pixel stand at their mean instead of aliasing, each
// offset from the last (`NOISE_OCTAVE_OFFSET`).
fn lattice_fbm(base: vec2u, p: vec2f, count: u32, first: u32, salt: u32, footprint: f32) -> f32 {
  let scale = 1u << first;
  var b = base * scale;
  var q = p * f32(scale);
  var n = count * scale;
  var sum = 0.0;
  var amp = 0.5;
  var reach = footprint * f32(scale);
  for (var k = 0u; k < NOISE_OCTAVES; k++) {
    if (reach > 0.5) {
      sum += amp * 0.5;
    } else {
      sum += amp * lattice_noise(b, q + f32(k) * NOISE_OCTAVE_OFFSET, n - 1u, salt + first + k);
    }
    reach *= 2.0;
    b *= 2u;
    q *= 2.0;
    n *= 2u;
    amp *= 0.5;
  }
  return sum;
}
