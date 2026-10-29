// What the cloud layers share. A shader including it declares `cloud_tile`
// and `cloud_tile_sampler` in a group of its own.

// A lattice unit is `CLOUD_LATTICE_CELLS` cells, which build.rs writes from
// pxl-spec-core param.rs `param::cloud::LATTICE_SHAMT`.

// On the map under it, so a cloud stays over its ground wherever the eye is.
fn cloud_lattice(point: vec3f) -> vec2f {
  return clouds_frac() + (point.xy + clouds_drift()) / f32(CLOUD_LATTICE_CELLS);
}

// Read at the footprint's mip, so a pixel wider than a cloud is as opaque as
// its covered share: the tile `finer` times smaller over the lattice. The base's share is scaled
// with the rest, so the pattern stays on its ground as the origin moves.
fn cloud_opacity_finer(p: vec2f, footprint: f32, finer: u32) -> f32 {
  let texels_per_unit = f32(CLOUD_TILE_TEXELS) / f32(CLOUD_TILE_UNITS);
  let base = (clouds_base() % vec2u(CLOUD_TILE_UNITS)) * finer % vec2u(CLOUD_TILE_UNITS);
  let uv = (vec2f(base) + p * f32(finer)) / f32(CLOUD_TILE_UNITS);
  return textureSampleLevel(cloud_tile, cloud_tile_sampler, uv, log2(max(footprint * f32(finer) * texels_per_unit, 1.0))).r;
}

// How much of a layer's field shows at lattice point `p`, its own per `salt`.
// The base is split on the coarse lattice, so the weather stays on its ground
// as the origin moves, and wraps with the map.
fn weather(p: vec2f, salt: u32) -> f32 {
  let base = clouds_base();
  let at = (vec2f(base % vec2u(CLOUD_WEATHER_CELL)) + p) / f32(CLOUD_WEATHER_CELL);
  let v = lattice_noise(base / CLOUD_WEATHER_CELL, at, clouds_cnt() / CLOUD_WEATHER_CELL - 1u, salt);
  let even = 1.0 / (1.0 + exp(-LOGISTIC_SCALE * (v - 0.5) / CLOUD_WEATHER_SIGMA));
  return smoothstep(CLOUD_WEATHER_CLEAR, CLOUD_WEATHER_CROWDED, even);
}

// Thins out over the reach's farther part.
fn reach_fade(t: f32, reach: f32) -> f32 {
  return 1.0 - smoothstep(CLOUD_REACH_FADE, 1.0, t / reach);
}

