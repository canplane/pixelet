// pxl-render/frame/shader/paint.wgsl

// What a block's face is painted, wherever it is drawn from: the chunks'
// fragments and the screen's rays both fill a `Surface` and call `paint`, so
// the two cannot paint one face differently. Its swatch or its layers' taps,
// the creases and corners its face bits name, lit by `shade`.

@group(0) @binding(4) var layers: texture_2d_array<f32>;
// Top, side and bottom slots in 10 bits each; NO_SLOT until an image arrives.
@group(0) @binding(5) var<storage, read> layer_slot: array<u32>;
@group(0) @binding(6) var layer_sampler: sampler;

// A tap past the last layer means the vertex color.
const NO_LAYER: f32 = 255.0;
const NO_SLOT: u32 = 0x3ffu;
const SLOT_BITS: u32 = 10u;
const IMAGE_TOP: u32 = 0u;
const IMAGE_SIDE: u32 = 1u;
const IMAGE_BOTTOM: u32 = 2u;

// A face as it is painted: its swatch and four layer taps, its normal, where
// in its box the point is and the box's size, the cell it is drawn at, its
// face bits (pxl-portable mask.rs), where it is in the world, its texture coordinate
// with that coordinate's and the box position's screen gradients, the screen's
// ambient occlusion there, and the air between it and the eye.
struct Surface {
  rgb: vec3f,
  taps: vec4f,
  normal: vec3f,
  local_pos: vec3f,
  box_scale: vec3f,
  cell: f32,
  faces: u32,
  world_pos: vec3f,
  box_uv: vec2f,
  ddx: vec2f,
  ddy: vec2f,
  pixel: vec3f,
  screen_open: f32,
  seen: Seen,
};

fn face_bit(faces: u32, bit: u32) -> bool {
  return (faces & bit) != 0u;
}

fn lateral_bit(x_axis: bool, positive: bool, base: u32) -> u32 {
  var k: u32;
  if (x_axis) {
    k = select(2u, 1u, positive);
  } else {
    k = select(8u, 4u, positive);
  }
  return base * k;
}

fn crease_dist(d: f32, convex: bool, concave: bool) -> f32 {
  return select(1e9, d, convex || concave);
}

// An edge is a crease when the face that way also exists or the cell at
// (d + n) is solid. Flat ground has no creases.
// `pixel` is the units one of the surface's pixels spans along each axis: an
// edge is a line of the same pixels whatever the cube's size or distance.
fn edge_factor(local_pos: vec3f, box_scale: vec3f, pixel: vec3f, normal: vec3f, faces: u32) -> f32 {
  if (frame_edge_strength() <= 0.0) {
    return 0.0;
  }
  let is_side = abs(normal.z) < 0.5;
  let dist_to_edge = (0.5 - abs(local_pos)) * box_scale / max(pixel, vec3f(1e-9));
  let px = local_pos.x > 0.0;
  let py = local_pos.y > 0.0;
  let pz = local_pos.z > 0.0;
  let x_dom = abs(normal.x) > 0.5;
  var n_positive: bool;
  if (x_dom) {
    n_positive = normal.x > 0.0;
  } else {
    n_positive = normal.y > 0.0;
  }
  var d = 1e9;
  if (!is_side) {
    let rise_or_dip = select(DIPS_BASE, RISES_BASE, pz);
    d = min(d, crease_dist(dist_to_edge.x, face_bit(faces, lateral_bit(true, px, CREASE_BASE)), face_bit(faces, lateral_bit(true, px, rise_or_dip))));
    d = min(d, crease_dist(dist_to_edge.y, face_bit(faces, lateral_bit(false, py, CREASE_BASE)), face_bit(faces, lateral_bit(false, py, rise_or_dip))));
  } else {
    let own = lateral_bit(x_dom, n_positive, FACE_BASE);
    d = min(d, crease_dist(dist_to_edge.z, !face_bit(faces, select(CAP_NZ, CAP_PZ, pz)), face_bit(faces, own * select(DIPS_BASE, RISES_BASE, pz))));
    let sx = select(px, n_positive, x_dom);
    let sy = select(n_positive, py, x_dom);
    var diag: u32;
    if (sx) {
      diag = select(DIAG_PN, DIAG_PP, sy);
    } else {
      diag = select(DIAG_NN, DIAG_NP, sy);
    }
    let horiz = select(dist_to_edge.x, dist_to_edge.y, x_dom);
    var horiz_convex: bool;
    if (x_dom) {
      horiz_convex = face_bit(faces, lateral_bit(false, py, CREASE_BASE));
    } else {
      horiz_convex = face_bit(faces, lateral_bit(true, px, CREASE_BASE));
    }
    d = min(d, crease_dist(horiz, horiz_convex, face_bit(faces, diag)));
  }
  let t = 1.0 - smoothstep(0.0, frame_edge_width(), d);
  return t * t;
}

// #1a1006 as a ratio: a warm contact shadow rather than gray.
const AO_TINT: vec3f = vec3f(1.0, 0.615, 0.231);

// Corner occlusion from the mesher's face bits: each concave edge darkens over
// one of the mesh's cells, `d` in them, and no further, since a box knows only
// the cells beside it: a reach past them was cut where the next box began.
// Squared, not smoothstep: a smoothstep sits flat against the wall and reads as
// a dark strip with an edge rather than a gradient.
fn occlusion_of(d: f32) -> f32 {
  let t = 1.0 - clamp(d, 0.0, 1.0);
  return t * t;
}

fn occlusion(local_pos: vec3f, box_scale: vec3f, cell: f32, normal: vec3f, faces: u32) -> f32 {
  if (frame_ao_strength() <= 0.0) {
    return 0.0;
  }
  let is_side = abs(normal.z) < 0.5;
  let to_pos = (0.5 - local_pos) * box_scale / cell;
  let to_neg = (0.5 + local_pos) * box_scale / cell;
  let x_dom = abs(normal.x) > 0.5;
  var n_positive: bool;
  if (x_dom) {
    n_positive = normal.x > 0.0;
  } else {
    n_positive = normal.y > 0.0;
  }
  // Each edge takes its share of what the last left. Summing saturated: the
  // inside of a corner went flat at its darkest.
  var open = 1.0;
  if (!is_side) {
    let base = select(DIPS_BASE, RISES_BASE, normal.z > 0.0);
    if (face_bit(faces, lateral_bit(true, true, base))) {
      open *= 1.0 - occlusion_of(to_pos.x);
    }
    if (face_bit(faces, lateral_bit(true, false, base))) {
      open *= 1.0 - occlusion_of(to_neg.x);
    }
    if (face_bit(faces, lateral_bit(false, true, base))) {
      open *= 1.0 - occlusion_of(to_pos.y);
    }
    if (face_bit(faces, lateral_bit(false, false, base))) {
      open *= 1.0 - occlusion_of(to_neg.y);
    }
    if (normal.z > 0.0) {
      if (face_bit(faces, DIAG_RISES_PP)) {
        open *= 1.0 - occlusion_of(length(vec2f(to_pos.x, to_pos.y)));
      }
      if (face_bit(faces, DIAG_RISES_PN)) {
        open *= 1.0 - occlusion_of(length(vec2f(to_pos.x, to_neg.y)));
      }
      if (face_bit(faces, DIAG_RISES_NP)) {
        open *= 1.0 - occlusion_of(length(vec2f(to_neg.x, to_pos.y)));
      }
      if (face_bit(faces, DIAG_RISES_NN)) {
        open *= 1.0 - occlusion_of(length(vec2f(to_neg.x, to_neg.y)));
      }
    }
  } else {
    let own = lateral_bit(x_dom, n_positive, FACE_BASE);
    if (face_bit(faces, own * RISES_BASE)) {
      open *= 1.0 - occlusion_of(to_pos.z);
    }
    if (face_bit(faces, own * DIPS_BASE)) {
      open *= 1.0 - occlusion_of(to_neg.z);
    }
    if (x_dom) {
      if (face_bit(faces, select(DIAG_NP, DIAG_PP, n_positive))) {
        open *= 1.0 - occlusion_of(to_pos.y);
      }
      if (face_bit(faces, select(DIAG_NN, DIAG_PN, n_positive))) {
        open *= 1.0 - occlusion_of(to_neg.y);
      }
    } else {
      if (face_bit(faces, select(DIAG_PN, DIAG_PP, n_positive))) {
        open *= 1.0 - occlusion_of(to_pos.x);
      }
      if (face_bit(faces, select(DIAG_NN, DIAG_NP, n_positive))) {
        open *= 1.0 - occlusion_of(to_neg.x);
      }
    }
  }
  return 1.0 - open;
}

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
  var taps = s.rgb;
  if (textured) {
    let image = face_image(s.normal);
    taps = (sample_tap(s.taps.x, image, s.box_uv, s.ddx, s.ddy, s.rgb)
      + sample_tap(s.taps.y, image, s.box_uv, s.ddx, s.ddy, s.rgb)
      + sample_tap(s.taps.z, image, s.box_uv, s.ddx, s.ddy, s.rgb)
      + sample_tap(s.taps.w, image, s.box_uv, s.ddx, s.ddy, s.rgb)) * 0.25;
  }
  var base = taps;
  base = base * mix(1.0, 1.0 - frame_edge_strength(), edge_factor(s.local_pos, s.box_scale, s.pixel, s.normal, s.faces));

  let n = normalize(s.normal);
  let k = frame_ao_strength() * occlusion(s.local_pos, s.box_scale, s.cell, s.normal, s.faces);
  let open = (1.0 - k) * mix(vec3f(1.0), AO_TINT, k);
  return shade(base, n, s.world_pos, open * s.screen_open, s.seen);
}
