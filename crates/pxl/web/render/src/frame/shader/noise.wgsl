// Value noise on the map's own lattice: a hash of a lattice point, the
// lattice smoothly interpolated, and octaves of it summed — the clouds'
// shapes.

// A lattice point's value, from its integer index: a mix of the two words
// and the octave's salt through a 32-bit finalizer, so no two points of a
// world's lattice share one.
fn lattice_hash(i: vec2u, salt: u32) -> f32 {
  var h = (i.x * 0x8da6b343u) ^ (i.y * 0xd8163841u) ^ (salt * 0xcb1ab31fu);
  h ^= h >> 16u;
  h *= 0x7feb352du;
  h ^= h >> 15u;
  h *= 0x846ca68bu;
  h ^= h >> 16u;
  return f32(h >> 8u) / 16777216.0;
}

// Value noise over the map's own lattice: the point is the lattice index
// `base`, exact, and `p` lattice units past it, small. The index wraps at
// `mask + 1`, a power of two that is the lattice's count across the world,
// so the noise wraps where the map does.
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

// Four octaves of it from octave `first`, each twice the last's frequency,
// but none finer than half a pixel: `footprint` is how many lattice units a
// pixel spans (the point's fwidth), and an octave whose lattice is finer
// than that would only alias, so it and those past it stand at their mean.
// Exactly twice, so each octave's lattice is still the map's — the index
// doubles with it and nothing is rounded; a fraction of a unit added per
// octave keeps their lattice lines from coinciding, which a plain factor of
// two would otherwise show as a grid.
fn lattice_fbm(base: vec2u, p: vec2f, count: u32, first: u32, salt: u32, footprint: f32) -> f32 {
  let scale = 1u << first;
  var b = base * scale;
  var q = p * f32(scale);
  var n = count * scale;
  var sum = 0.0;
  var amp = 0.5;
  var reach = footprint * f32(scale);
  for (var k = 0u; k < 4u; k++) {
    if (reach > 0.5) {
      // Value noise's mean.
      sum += amp * 0.5;
    } else {
      sum += amp * lattice_noise(b, q + f32(k) * vec2f(0.37, 0.61), n - 1u, salt + first + k);
    }
    reach *= 2.0;
    b *= 2u;
    q *= 2.0;
    n *= 2u;
    amp *= 0.5;
  }
  return sum;
}
