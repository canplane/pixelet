@group(0) @binding(2) var cloud_tile: texture_2d<f32>;
@group(0) @binding(3) var cloud_tile_sampler: sampler;

// 2^12 cells, about 8 km at mid latitudes; a power of two so clouds wrap with the map.
const CLOUD_LATTICE_CELLS: u32 = 4096u;
// About 8 m/s at mid latitudes.
const CLOUD_WIND: vec2f = vec2f(3.0, 1.25);

// Taken back to the flat map so a cloud stays over its ground wherever the view is centered.
fn cloud_lattice(point: vec3f) -> vec2f {
  let on_map = off_globe_of(point, frame_planet(), frame_globe());
  let cells = frame_planet().xy + on_map + atmos_cloud_time() * CLOUD_WIND;
  return atmos_cloud_frac() + cells / f32(CLOUD_LATTICE_CELLS);
}

// Read at the footprint's mip, so a pixel wider than a cloud is as opaque as its covered share.
fn cloud_opacity(p: vec2f, footprint: f32) -> f32 {
  return cloud_opacity_finer(p, footprint, 1u);
}

// The tile `finer` times smaller over the lattice. The base's share is scaled
// with the rest, so the pattern stays on its ground as the origin moves.
fn cloud_opacity_finer(p: vec2f, footprint: f32, finer: u32) -> f32 {
  let texels_per_unit = f32(CLOUD_TILE_TEXELS) / f32(CLOUD_TILE_UNITS);
  let base = (atmos_cloud_base() % vec2u(CLOUD_TILE_UNITS)) * finer % vec2u(CLOUD_TILE_UNITS);
  let uv = (vec2f(base) + p * f32(finer)) / f32(CLOUD_TILE_UNITS);
  return textureSampleLevel(cloud_tile, cloud_tile_sampler, uv, log2(max(footprint * f32(finer) * texels_per_unit, 1.0))).r;
}

// Fades over the upper half of the climb to where the clouds are gone.
fn cloud_shown() -> f32 {
  return 1.0 - smoothstep(0.5 * atmos_cloud_gone_km(), atmos_cloud_gone_km(), atmos_camera_altitude());
}
