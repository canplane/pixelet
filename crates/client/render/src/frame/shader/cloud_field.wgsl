// What the cloud layers share. A shader including it declares `cloud_tile`
// and `cloud_tile_sampler` in a group of its own.

// 2^12 cells, about 8 km at mid latitudes; a power of two so clouds wrap with
// the map (look/cloud.rs `LATTICE_SHAMT`).
const CLOUD_LATTICE_CELLS: u32 = 4096u;

// Taken back to the flat map so a cloud stays over its ground wherever the view is centered.
fn cloud_lattice(point: vec3f) -> vec2f {
  let on_map = off_globe_of(point, frame_planet(), frame_globe());
  let cells = frame_planet().xy + on_map + clouds_drift();
  return clouds_frac() + cells / f32(CLOUD_LATTICE_CELLS);
}

// Read at the footprint's mip, so a pixel wider than a cloud is as opaque as its covered share.
fn cloud_opacity(p: vec2f, footprint: f32) -> f32 {
  return cloud_opacity_finer(p, footprint, 1u);
}

// The tile `finer` times smaller over the lattice. The base's share is scaled
// with the rest, so the pattern stays on its ground as the origin moves.
fn cloud_opacity_finer(p: vec2f, footprint: f32, finer: u32) -> f32 {
  let texels_per_unit = f32(CLOUD_TILE_TEXELS) / f32(CLOUD_TILE_UNITS);
  let base = (clouds_base() % vec2u(CLOUD_TILE_UNITS)) * finer % vec2u(CLOUD_TILE_UNITS);
  let uv = (vec2f(base) + p * f32(finer)) / f32(CLOUD_TILE_UNITS);
  return textureSampleLevel(cloud_tile, cloud_tile_sampler, uv, log2(max(footprint * f32(finer) * texels_per_unit, 1.0))).r;
}

// Where clouds gather: value noise over cells this many lattice units wide,
// about 64 km, spread even through the logistic and cut so some country stays
// clear and some is crowded, rather than every field equally dotted.
const WEATHER_CELL: u32 = 8u;
// The bilinear smoothstep value noise's deviation about its 0.5 mean, from its formula.
const WEATHER_SIGMA: f32 = 0.2145;
const WEATHER_CLEAR: f32 = 0.35;
const WEATHER_CROWDED: f32 = 0.75;

// How much of a layer's field shows at lattice point `p`, its own per `salt`.
// The base is split on the coarse lattice, so the weather stays on its ground
// as the origin moves, and wraps with the map.
fn weather(p: vec2f, salt: u32) -> f32 {
  let base = clouds_base();
  let at = (vec2f(base % vec2u(WEATHER_CELL)) + p) / f32(WEATHER_CELL);
  let v = lattice_noise(base / WEATHER_CELL, at, clouds_count() / WEATHER_CELL - 1u, salt);
  let even = 1.0 / (1.0 + exp(-LOGISTIC_SCALE * (v - 0.5) / WEATHER_SIGMA));
  return smoothstep(WEATHER_CLEAR, WEATHER_CROWDED, even);
}

// Where a layer `km` over the ground meets the horizon from the eye, in km.
fn cloud_reach(km: f32) -> f32 {
  let r = atmos_bottom_radius();
  let layer = r + km;
  return sqrt(max(camera_r() * camera_r() - r * r, 0.0)) + sqrt(layer * layer - r * r);
}

// Thins out over the reach's farther part.
fn reach_fade(t: f32, reach: f32) -> f32 {
  return 1.0 - smoothstep(0.6, 1.0, t / reach);
}

const CLOUD_ALBEDO: f32 = 0.9;
