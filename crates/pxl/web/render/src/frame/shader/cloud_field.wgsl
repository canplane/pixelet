// The cloud layer as a field over the planet, for the clouds' pass
// (clouds.wgsl): where the layer is (the air's uniform's `cloud_*`,
// gpu/clouds.rs's `field`), how much of the light it stops over any point
// of it — read from the tile that is baked once (cloud_bake.wgsl) — and how
// much of it the camera's height shows.

@group(0) @binding(2) var cloud_tile: texture_2d<f32>;
@group(0) @binding(3) var cloud_tile_sampler: sampler;

// The size of the largest cloud shapes: the lattice the tile is drawn on,
// 2^12 cells, some eight kilometers in the middle latitudes. A power of two,
// so it divides the world and the clouds wrap with the map (gpu/clouds.rs's
// LATTICE_SHAMT).
const CLOUD_LATTICE_CELLS: u32 = 4096u;
// The wind that carries them, in cells a second: about eight meters a
// second, at the cell's size in the middle latitudes.
const CLOUD_WIND: vec2f = vec2f(3.0, 1.25);

// A point of the root frame, on the layer or anywhere over the planet, in
// lattice units past the origin's lattice point: taken back to the flat map
// (globe.wgsl's off_globe_of), so a cloud stays over the ground under it
// wherever the view is centered, and carried by the wind.
fn cloud_lattice(point: vec3f) -> vec2f {
  let on_map = off_globe_of(point, frame.planet, frame.globe);
  let cells = frame.planet.xy + on_map + atmos.cloud_time * CLOUD_WIND;
  return atmos.cloud_frac + cells / f32(CLOUD_LATTICE_CELLS);
}

// How much of the light through the layer it stops at lattice point `p`,
// `footprint` lattice units of it under a pixel: the tile the curve was
// baked into (cloud_bake.wgsl), read at the mip level that footprint asks
// for, so a pixel wider than a cloud is as opaque as the share of it the
// clouds under it cover.
fn cloud_opacity(p: vec2f, footprint: f32) -> f32 {
  let texels_per_unit = f32(CLOUD_TILE_TEXELS) / f32(CLOUD_TILE_UNITS);
  let uv = (vec2f(atmos.cloud_base % vec2u(CLOUD_TILE_UNITS)) + p) / f32(CLOUD_TILE_UNITS);
  return textureSampleLevel(cloud_tile, cloud_tile_sampler, uv, log2(max(footprint * texels_per_unit, 1.0))).r;
}

// How much of the layer is shown from the camera's height: all of it from
// under it, thinning from it up, none past `cloud_gone_km` — seen from
// above, flat noise over the ground reads as blots on it.
fn cloud_shown() -> f32 {
  return 1.0 - smoothstep(atmos.cloud_altitude_km, atmos.cloud_gone_km, atmos.camera_altitude);
}
