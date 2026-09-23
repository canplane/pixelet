// The cloud layer as a field over the planet, for the clouds' own pass
// (clouds.wgsl) and for the shadows they lay on the lit world
// (atmos_view.wgsl's seen_from_eye): where the layer is (the air's uniform's
// `cloud_*`, gpu/clouds.rs's `field`), and how thick it is over any point
// of it. One source, so a shadow lies under the cloud that casts it.

// How much of the sky the layer covers.
const CLOUD_COVER: f32 = 0.42;
// The size of the largest cloud shapes: the lattice the noise is drawn on,
// 2^12 cells, some eight kilometres in the middle latitudes. A power of two,
// so it divides the world and the clouds wrap with the map (gpu/clouds.rs's
// LATTICE_SHAMT).
const CLOUD_LATTICE_CELLS: u32 = 4096u;
// The wind that carries them, in cells a second: about eight metres a
// second, at the cell's size in the middle latitudes.
const CLOUD_WIND: vec2f = vec2f(3.0, 1.25);
// How thick a full cloud is to light, as optical depth: past a few, its
// underside is in its own shadow and grey, and its edges, thin, are bright.
const CLOUD_DEPTH: f32 = 6.0;

// A point of the root frame, on the layer or anywhere over the planet, in
// lattice units past the origin's lattice point: taken back to the flat map
// (globe.wgsl's off_globe_of), so a cloud stays over the ground under it
// wherever the view is centred, and carried by the wind.
fn cloud_lattice(point: vec3f) -> vec2f {
  let on_map = off_globe_of(point, frame.planet, frame.globe);
  let cells = frame.planet.xy + on_map + atmos.cloud_time * CLOUD_WIND;
  return atmos.cloud_frac + cells / f32(CLOUD_LATTICE_CELLS);
}

// The layer's optical depth at lattice point `p`, its octaves cut at
// `footprint` lattice units (noise.wgsl's lattice_fbm).
fn cloud_depth(p: vec2f, footprint: f32) -> f32 {
  let shape = lattice_fbm(atmos.cloud_base, p, atmos.cloud_count, 0u, 0u, footprint) * 0.75
    + lattice_fbm(atmos.cloud_base, p, atmos.cloud_count, 2u, 16u, footprint) * 0.25;
  return smoothstep(1.0 - CLOUD_COVER - 0.12, 1.0 - CLOUD_COVER + 0.18, shape) * CLOUD_DEPTH;
}

// How much of the layer is shown from the camera's height: all of it from
// under it, thinning from it up, none past `cloud_gone_km` — seen from
// above, flat noise over the ground reads as blots on it. Its shadows go
// with it: a shadow of a cloud the frame does not show would be a blot too.
fn cloud_shown() -> f32 {
  return 1.0 - smoothstep(atmos.cloud_altitude_km, atmos.cloud_gone_km, atmos.camera_altitude);
}
