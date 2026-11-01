// Shared by the AO passes and its apply: N8AO 1.10.1 (CC0) ported to WGSL and
// reversed depth, at its "performance" settings with four hemisphere samples,
// not eight: the sample pass is most of its cost. Everything is in view space.

// Read field by field from vec4 words, never as a struct: a struct here lost
// Safari's WebGPU its GPU process. The screen's size and the fog are the frame's.
@group(0) @binding(6) var<uniform> ao_words: array<vec4f, AO_VEC4S>;

// The blue-noise texture's side less one; a pixel reads it tiled.
const AO_NOISE_MASK: i32 = i32(AO_NOISE_SIDE) - 1;

// Reversed depth `d`; `uv` y down.
fn view_pos(d: f32, uv: vec2f) -> vec3f {
  let p = ao_inv_proj() * vec4f(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, d, 1.0);
  return p.xyz / p.w;
}

// The side of the cell drawn at view position `pos`, in units: the camera
// grid's at its distance, a zoom's cell drawn from `GRID_SPLIT` of its sides
// on (pxl-spec-core grid.rs `cell_zoom_at`), the ground's leaf where that is
// finer. The occlusion's reach is in these, so it closes a crease over the
// same share of a cell at every zoom, as the corners' do (screen_ray.wgsl
// `creases`).
fn ao_cell(pos: vec3f) -> f32 {
  let leaf = exp2(f32(i32(UNIT_ZOOM) - i32(GROUND_CELL)));
  return max(leaf, exp2(floor(log2(length(pos) / GRID_SPLIT))));
}

fn view_dist(d: f32) -> f32 {
  let p = ao_inv_proj() * vec4f(0.0, 0.0, d, 1.0);
  return -p.z / p.w;
}
