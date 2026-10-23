// pxl-render/frame/shader/paint.wgsl

// What a block's face is painted, wherever it is drawn from: the ghost's
// boxes (box.wgsl) and the screen's rays (screen_rays.wgsl) both fill a
// `Surface` and call `paint`, so the two cannot paint one face differently.
// Its swatch or its layers' four taps, darkened by its own edges as much as its
// caller says, lit by `shade`: a block is painted from its own words.

@group(0) @binding(4) var layers: texture_2d_array<f32>;
// Top, side and bottom slots in 10 bits each; NO_SLOT until an image arrives.
@group(0) @binding(5) var<storage, read> layer_slot: array<u32>;
@group(0) @binding(6) var layer_sampler: sampler;

// NO_LAYER, a layer past the last, means the vertex color; the slots'
// words are frame/shader/words.rs's.

// A face's outward normal by its number, 2·axis plus one down the axis, the
// order of a ray's face (pxl-spec-core ray.rs).
const FACE_NORMAL = array<vec3f, 6>(
  vec3f(1.0, 0.0, 0.0), vec3f(-1.0, 0.0, 0.0),
  vec3f(0.0, 1.0, 0.0), vec3f(0.0, -1.0, 0.0),
  vec3f(0.0, 0.0, 1.0), vec3f(0.0, 0.0, -1.0),
);

fn srgb_to_linear(c: vec3f) -> vec3f {
  let low = c / 12.92;
  let high = pow((c + 0.055) / 1.055, vec3f(2.4));
  return select(high, low, c <= vec3f(0.04045));
}

// A face as it is painted: its swatch and four layer taps, its normal, how
// much its own edges darken it there (0 to 1), where it is in the world, its
// texture coordinate with that coordinate's screen gradients, the screen's
// ambient occlusion there, and the air between it and the eye.
struct Surface {
  rgb: vec3f,
  taps: vec4f,
  normal: vec3f,
  edge: f32,
  world_pos: vec3f,
  box_uv: vec2f,
  ddx: vec2f,
  ddy: vec2f,
  screen_open: f32,
  seen: Seen,
};

// A block's face wears its own image for that face, at any depth: a cell
// under the surface is its own block, the stratum's.
fn face_image(normal: vec3f) -> u32 {
  if (normal.z > 0.5) {
    return IMAGE_TOP;
  }
  if (normal.z < -0.5) {
    return IMAGE_BOTTOM;
  }
  return IMAGE_SIDE;
}

// Gradients are passed in because the read is inside a branch.
fn sample_tap(layer: f32, image: u32, uv: vec2f, ddx: vec2f, ddy: vec2f, fallback: vec3f) -> vec3f {
  let l = u32(clamp(layer, 0.0, 255.0) + 0.5);
  if (l >= arrayLength(&layer_slot)) {
    return fallback;
  }
  let slot = (layer_slot[l] >> (image * SLOT_BITS)) & NO_SLOT;
  if (slot == NO_SLOT) {
    return fallback;
  }
  return textureSampleGrad(layers, layer_sampler, uv, i32(slot), ddx, ddy).rgb;
}

fn paint(s: Surface) -> vec3f {
  // Only leaf cells carry layers; the samples are skipped, not taken and discarded.
  let textured = s.taps.x < NO_LAYER - 0.5 && frame_low_spec() < 0.5;
  var albedo = s.rgb;
  if (textured) {
    let image = face_image(s.normal);
    albedo = (sample_tap(s.taps.x, image, s.box_uv, s.ddx, s.ddy, s.rgb)
      + sample_tap(s.taps.y, image, s.box_uv, s.ddx, s.ddy, s.rgb)
      + sample_tap(s.taps.z, image, s.box_uv, s.ddx, s.ddy, s.rgb)
      + sample_tap(s.taps.w, image, s.box_uv, s.ddx, s.ddy, s.rgb)) * 0.25;
  }
  let base = albedo * mix(1.0, 1.0 - frame_edge_strength(), s.edge);
  return shade(base, normalize(s.normal), s.world_pos, vec3f(s.screen_open), s.seen);
}
