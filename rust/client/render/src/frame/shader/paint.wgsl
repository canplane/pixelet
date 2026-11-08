// pxl-render/frame/shader/paint.wgsl

// How a cell's face is painted, wherever it is drawn from. The ghost's boxes
// (box.wgsl) and the screen's rays (screen_ray.wgsl) both fill a `Surface` and
// call `paint`, so the two cannot paint one face differently. A face takes its
// swatch or its layer's tap, is darkened by its own edges as much as its caller
// says, and is lit by `shade`. A cell is painted from its own words alone.

@group(0) @binding(4) var layers: texture_2d_array<f32>;
// Each layer's slots for its top, side and bottom images, `SLOT_BITS` bits
// each. A slot is `NO_SLOT` until its image arrives.
@group(0) @binding(5) var<storage, read> layer_slot: array<u32>;
@group(0) @binding(6) var layer_sampler: sampler;

// A face given `NO_LAYER`, a layer past the last, takes its swatch, the vertex
// color. frame/shader/word.rs lays out the slots' words.

// A face's outward normal by its number, which is 2·axis plus one where the
// normal points down the axis, as a ray numbers faces (pxl-engine ray.rs).
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

// A face as it is painted: its swatch and its layer, its normal, how much its
// own edges darken it there (0 to 1), where it is in the world, its texture
// coordinate with that coordinate's screen gradients, and how open it is to the
// sky there. The openness is the screen's ambient occlusion, and its corners'
// where the caller knows them. The air between the face and the eye, and the
// lights there, are `paint`'s own arguments.
struct Surface {
  rgb: vec3f,
  layer: f32,
  normal: vec3f,
  edge: f32,
  world_pos: vec3f,
  box_uv: vec2f,
  ddx: vec2f,
  ddy: vec2f,
  ao: f32,
};

// A cell's face wears its own asset's image for that side, at any depth. A cell
// under the surface holds an asset of its own, its stratum's.
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
  let l = u32(clamp(layer, 0.0, NO_LAYER) + 0.5);
  if (l >= arrayLength(&layer_slot)) {
    return fallback;
  }
  let slot = (layer_slot[l] >> (image * SLOT_BITS)) & NO_SLOT;
  if (slot == NO_SLOT) {
    return fallback;
  }
  return textureSampleGrad(layers, layer_sampler, uv, i32(slot), ddx, ddy).rgb;
}

// A face's texture coordinate at the world point `p` it shows, for a face whose
// normal is `normal`. The texture repeats once a unit. A top maps by X and Y,
// and a side by its run and down its height.
fn face_uv(p: vec3f, normal: vec3f) -> vec2f {
  if (abs(normal.z) > 0.5) {
    return p.xy;
  }
  return select(vec2f(p.x, -p.z), vec2f(p.y, -p.z), abs(normal.x) > 0.5);
}

// How much a face's edge darkens it `d` units from the edge, where a drawn
// pixel spans `px` units that way. The line is `EFFECT_EDGE_WIDTH` units wide
// whatever the cell's size, so only perspective thins it far off. It fades out
// across its width and one pixel past it. Once it is thinner than a pixel, it
// is as faint as the share of the pixel it covers.
fn line_at(d: f32, px: f32) -> f32 {
  let t = 1.0 - smoothstep(0.0, EFFECT_EDGE_WIDTH + px, d);
  return t * t * min(EFFECT_EDGE_WIDTH / px, 1.0);
}

fn paint(s: Surface, seen: Air, l: Light) -> vec3f {
  // Only leaf cells carry layers. On a low spec the taps are skipped, and the
  // face takes its swatch.
  let textured = s.layer < NO_LAYER - 0.5 && frame_low_spec() < 0.5;
  var albedo = s.rgb;
  if (textured) {
    let image = face_image(s.normal);
    albedo = sample_tap(s.layer, image, s.box_uv, s.ddx, s.ddy, s.rgb);
  }
  let base = albedo * mix(1.0, 1.0 - frame_edge_strength(), s.edge);
  let n = normalize(s.normal);
  return shade(base, n, s.world_pos, shadow_visibility(s.world_pos, n), s.ao, seen, l);
}
