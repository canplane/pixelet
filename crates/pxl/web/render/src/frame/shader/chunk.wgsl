// The chunk mesh: the mesher's boxes as they lie in a block, each unfolded
// here into a cube's six faces, a face whose bit the mask lacks collapsed to
// nothing (chunk_mesher.rs's BOX_WORDS). Every rule here was chunk_material.ts's injected GLSL over
// three's standard material (b9e31004) — the
// real-size tile UV from a box's own corner and scale, the edge shading
// from the mesher's face bits, the screen-space dither a blend is punched
// with — with the lighting three did for us written out in shade.wgsl. No
// tone mapping: three applied it only when drawing to the screen, and the
// shipped path drew into the composer's target.

struct Draw {
  translation: vec3f,
  visible: f32,
  opacity: f32,
  // The cell a box of this draw is measured in, in units: a leaf's own cell
  // is one, a tier's is its step, and a level's is that shifted by the
  // level. A box holds its corner and width in steps of it (Box), and its
  // floor and height in steps of `z_step`, from the mesh's own floor, which
  // is in the translation.
  unit: f32,
  z_step: f32,
  // Which of the two box formats this draw's group holds, in words
  // (chunk_mesher.rs's FINE_BOX_WORDS): a group is one format throughout.
  box_words: f32,
};

@group(0) @binding(3) var<storage, read> draws: array<Draw>;
@group(0) @binding(4) var layers: texture_2d_array<f32>;
// Where each layer's two faces are in the array: the top's slot in the low
// half of its word, the side's in the high (gpu/layers.rs). NO_SLOT for a
// face the pack ships no tile for, and for every face until it arrives.
@group(0) @binding(5) var<storage, read> layer_slot: array<u32>;
@group(0) @binding(6) var layer_sampler: sampler;

// volume.rs's NO_LAYER: a tap past the last layer means the vertex colour.
const NO_LAYER: f32 = 255.0;
// gpu/layers.rs's NO_SLOT.
const NO_SLOT: u32 = 0xffffu;

// One box of the mesh, unfolded from the words the mesher packed
// (mesh/chunk_mesher.rs). Its corner and width across x and y are steps of
// its own cell, which the draw says the size of; its floor and height are
// steps of the mesh's own, because a chunk is not divided in z, so what a
// mesh spans there is its own and the draw says what one step of it is.
struct Box {
  min_xy: vec2f,
  size_xy: vec2f,
  z: vec2f,
  rgb: u32,
  taps: u32,
  faces: u32,
};

// The steps a cell is cut into across x and y: chunk_mesher.rs's XY_STEPS
// for a leaf's own box, COARSE_XY_STEPS for one that merges cells.
const XY_STEPS: f32 = 256.0;
const COARSE_XY_STEPS: f32 = 8.0;
// NO_LAYER in all four taps: what a box that merges cells wears, and the
// reason it need not carry them.
const NO_TAPS: u32 = 0xffffffffu;

@group(1) @binding(0) var<storage, read> boxes: array<u32>;

fn box_pair(word: u32, step: f32) -> vec2f {
  return vec2f(f32(word & 0xffffu), f32(word >> 16u)) * step;
}

// Box `i` of the group this draw reads, in the draw's own format. The floor
// and the height sit in the same word in both.
fn box_at(i: u32, d: Draw) -> Box {
  var b: Box;
  let base = i * u32(d.box_words);
  b.z = box_pair(boxes[base + 2u], d.z_step);
  if (d.box_words > 5.0) {
    let step = d.unit / XY_STEPS;
    b.min_xy = box_pair(boxes[base], step);
    b.size_xy = box_pair(boxes[base + 1u], step);
    b.rgb = boxes[base + 3u];
    b.taps = boxes[base + 4u];
    b.faces = boxes[base + 5u];
    return b;
  }
  let step = d.unit / COARSE_XY_STEPS;
  b.faces = boxes[base];
  b.rgb = boxes[base + 1u];
  let w = boxes[base + 3u];
  b.min_xy = vec2f(f32(w & 0xffu), f32((w >> 8u) & 0xffu)) * step;
  // A width is held one short, so a whole mesh across fits in a byte.
  b.size_xy = (vec2f(f32((w >> 16u) & 0xffu), f32((w >> 24u) & 0xffu)) + 1.0) * step;
  b.taps = NO_TAPS;
  return b;
}

// A face's outward normal and its four corners in the box's [-0.5, 0.5]
// cube, wound counter-clockwise from outside: +x, -x, +y, -y, +z, -z, the
// order of the mask's face bits.
const FACE_NORMAL = array<vec3f, 6>(
  vec3f(1.0, 0.0, 0.0), vec3f(-1.0, 0.0, 0.0),
  vec3f(0.0, 1.0, 0.0), vec3f(0.0, -1.0, 0.0),
  vec3f(0.0, 0.0, 1.0), vec3f(0.0, 0.0, -1.0),
);
const FACE_CORNER = array<vec3f, 24>(
  vec3f(0.5, -0.5, -0.5), vec3f(0.5, 0.5, -0.5), vec3f(0.5, 0.5, 0.5), vec3f(0.5, -0.5, 0.5),
  vec3f(-0.5, -0.5, -0.5), vec3f(-0.5, -0.5, 0.5), vec3f(-0.5, 0.5, 0.5), vec3f(-0.5, 0.5, -0.5),
  vec3f(-0.5, 0.5, -0.5), vec3f(-0.5, 0.5, 0.5), vec3f(0.5, 0.5, 0.5), vec3f(0.5, 0.5, -0.5),
  vec3f(-0.5, -0.5, -0.5), vec3f(0.5, -0.5, -0.5), vec3f(0.5, -0.5, 0.5), vec3f(-0.5, -0.5, 0.5),
  vec3f(-0.5, -0.5, 0.5), vec3f(0.5, -0.5, 0.5), vec3f(0.5, 0.5, 0.5), vec3f(-0.5, 0.5, 0.5),
  vec3f(-0.5, -0.5, -0.5), vec3f(-0.5, 0.5, -0.5), vec3f(0.5, 0.5, -0.5), vec3f(0.5, -0.5, -0.5),
);
// A face's two triangles over its four corners.
const FACE_TRIANGLES = array<u32, 6>(0u, 1u, 2u, 0u, 2u, 3u);
// Vertices a box is drawn with: six faces of two triangles.
const BOX_VERTICES: u32 = 36u;

// The sRGB transfer undone, per channel.
fn srgb_to_linear(c: vec3f) -> vec3f {
  let low = c / 12.92;
  let high = pow((c + 0.055) / 1.055, vec3f(2.4));
  return select(high, low, c <= vec3f(0.04045));
}

struct VertexOut {
  @builtin(position) clip: vec4f,
  @location(0) rgb: vec3f,
  @location(1) layer_taps: vec4f,
  @location(2) box_uv: vec2f,
  @location(3) local_pos: vec3f,
  @location(4) box_scale: vec3f,
  @location(5) normal: vec3f,
  @location(6) world_pos: vec3f,
  @location(7) @interpolate(flat) faces: u32,
  @location(8) @interpolate(flat) draw: u32,
  // The air in front of the vertex (atmos_view.wgsl's Seen).
  @location(9) through: vec3f,
  @location(10) scattered: vec3f,
  @location(11) past: vec3f,
  @location(12) fill: vec3f,
};

@vertex
fn vs(@builtin(vertex_index) vertex: u32, @builtin(instance_index) draw: u32) -> VertexOut {
  let d = draws[draw];
  let b = box_at(vertex / BOX_VERTICES, d);
  let k = vertex % BOX_VERTICES;
  let face = k / 6u;
  var o: VertexOut;
  // A face the mask leaves out: every vertex of it at one point, a triangle
  // of no area, which the rasterizer drops.
  if ((b.faces & (1u << face)) == 0u) {
    o.clip = vec4f(0.0, 0.0, 0.0, 1.0);
    return o;
  }
  let local = FACE_CORNER[face * 4u + FACE_TRIANGLES[k % 6u]];
  let size = vec3f(b.size_xy, b.z.y);
  let position = vec3f(b.min_xy, b.z.x) + size * 0.5 + local * size;
  let normal = FACE_NORMAL[face];
  let world = on_globe(position + d.translation);
  o.clip = frame.view_proj * vec4f(world, 1.0);
  o.rgb = srgb_to_linear(unpack4x8unorm(b.rgb).xyz);
  o.layer_taps = unpack4x8unorm(b.taps) * 255.0;
  o.local_pos = local;
  o.box_scale = size;
  o.normal = normal;
  o.faces = b.faces;
  o.draw = draw;
  o.world_pos = world;
  let seen = seen_at_vertex(world);
  o.through = seen.through;
  o.scattered = seen.scattered;
  o.past = seen.past;
  o.fill = seen.fill;
  // Real-size tile UV: a box's own [-0.5, 0.5] corner times its real size
  // per axis, so a box spanning many cells repeats the tile once a cell.
  // Side faces put height in v with the box's top at v = 0, the image's
  // top row; the array is sampled unflipped.
  let is_side = abs(normal.z) < 0.5;
  if (is_side) {
    if (abs(normal.x) > 0.5) {
      o.box_uv = vec2f((local.y + 0.5) * size.y, (0.5 - local.z) * size.z);
    } else {
      o.box_uv = vec2f((local.x + 0.5) * size.x, (0.5 - local.z) * size.z);
    }
  } else {
    o.box_uv = vec2f((local.x + 0.5) * size.x, (local.y + 0.5) * size.y);
  }
  return o;
}

// Interleaved gradient noise (Jimenez): screen-space on purpose, so a
// cross-fade reads as a fade and not as a texture stuck to the surface.
fn dither_threshold(frag: vec2f) -> f32 {
  return fract(52.9829189 * fract(dot(frag, vec2f(0.06711056, 0.00583715))));
}

fn face_bit(faces: u32, bit: u32) -> bool {
  return (faces & bit) != 0u;
}

// A lateral's bit from its group's base (faces.wgsl): +X, -X, +Y, -Y.
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

// Edge shading from one rule: a face's edge toward an in-plane direction
// is a crease when the face that way also exists (a silhouette) or the
// cell at (d + n) is solid (the surface turns inward). Flat ground has no
// creases and stays seamless.
fn edge_factor(local_pos: vec3f, box_scale: vec3f, normal: vec3f, faces: u32) -> f32 {
  if (frame.edge_strength <= 0.0) {
    return 0.0;
  }
  let is_side = abs(normal.z) < 0.5;
  let dist_to_edge = (0.5 - abs(local_pos)) * box_scale;
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
  let t = 1.0 - smoothstep(0.0, frame.edge_width, d);
  return t * t;
}

// Corner occlusion from the same bits: a face loses ambient light where the
// surface turns inward — a top face along a lateral that rises above it, a
// bottom face along one that dips below, a side face at the end where the
// column in front of it continues, and the inside of a corner, which the
// diagonal bit names. A silhouette edge faces open air and takes none. Each
// concave edge contributes a falloff over AO_REACH cells, a smoothstep so it
// leaves the crease gently; a corner sums two and reads darker, which is what
// a corner does. Screen-space AO finds the same creases by probing depth
// (gpu/ao.rs, the `screen` mode); the mesher already knows them, so this
// costs the bits and a few ops. The reach and the profile were widened and
// softened from 0.55 and a square once the two could be switched side by
// side: the screen-space pass reaches 1.5 units and its occlusion is a
// sample count that thins out over the whole radius.
const AO_REACH: f32 = 1.5;
// The occluded light's hue: #1a1006 as a ratio, red kept — the warm dark of
// a contact shadow rather than a grey one, which is what the screen-space
// pass this replaces was tuned to.
const AO_TINT: vec3f = vec3f(1.0, 0.615, 0.231);

// The falloff from one concave edge, over AO_REACH cells. Squared and not a
// smoothstep: a smoothstep is flat at both ends, so the shading sat against
// the wall at nearly its full value and then fell away over the middle,
// which reads as a dark strip with an edge rather than as a gradient. This
// leaves the wall at once and tapers to nothing.
fn occlusion_of(d: f32) -> f32 {
  let t = 1.0 - clamp(d / AO_REACH, 0.0, 1.0);
  return t * t;
}

fn occlusion(local_pos: vec3f, box_scale: vec3f, normal: vec3f, faces: u32) -> f32 {
  if (frame.ao_strength <= 0.0) {
    return 0.0;
  }
  let is_side = abs(normal.z) < 0.5;
  // Every concave edge of the face counts, each at its own distance — the
  // reach runs past the middle of a cell, so a face between two walls is
  // shaded from both and a face with one is shaded from that one only,
  // with no seam where the nearer edge would change.
  let to_pos = (0.5 - local_pos) * box_scale;
  let to_neg = (0.5 + local_pos) * box_scale;
  let x_dom = abs(normal.x) > 0.5;
  var n_positive: bool;
  if (x_dom) {
    n_positive = normal.x > 0.0;
  } else {
    n_positive = normal.y > 0.0;
  }
  // What the face still sees, each edge taking its share of what the last
  // one left. Summing them instead saturated: two edges at half their
  // strength already reached the clamp, so the inside of a corner went flat
  // at its darkest over the whole area where both reach. This darkens with
  // every edge and never arrives.
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
    // The corners, at the distance to the corner itself: a cube touching
    // only there shades less than one along a whole edge, which falls out
    // of the distance being the longer one.
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
    // The inside corners at the face's two ends, which the diagonal bits
    // name by the quadrant the neighbour is in.
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

// The gradients are the caller's: a tap is read inside a branch — a box
// with no layer skips the read altogether — and a sample that takes its own
// gradients may only be called where every fragment of the quad takes it.
fn sample_tap(layer: f32, is_side: bool, uv: vec2f, ddx: vec2f, ddy: vec2f, fallback: vec3f) -> vec3f {
  let l = u32(clamp(layer, 0.0, 255.0) + 0.5);
  if (l >= arrayLength(&layer_slot)) {
    return fallback;
  }
  let word = layer_slot[l];
  let slot = select(word & 0xffffu, word >> 16u, is_side);
  if (slot == NO_SLOT) {
    return fallback;
  }
  return textureSampleGrad(layers, layer_sampler, uv, i32(slot), ddx, ddy).rgb;
}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  let d = draws[i.draw];
  // A box with no layer of its own is its colour and nothing else: that is
  // every tier over a leaf's own cells, which are the only ones the texture
  // pack is for (mesh/chunk_mesher.rs). The four samples are skipped with
  // it, not taken and thrown away.
  let textured = i.layer_taps.x < NO_LAYER - 0.5 && frame.low_spec < 0.5;
  let ddx = dpdx(i.box_uv);
  let ddy = dpdy(i.box_uv);
  var taps = i.rgb;
  if (textured) {
    let is_side = abs(i.normal.z) < 0.5;
    taps = (sample_tap(i.layer_taps.x, is_side, i.box_uv, ddx, ddy, i.rgb)
      + sample_tap(i.layer_taps.y, is_side, i.box_uv, ddx, ddy, i.rgb)
      + sample_tap(i.layer_taps.z, is_side, i.box_uv, ddx, ddy, i.rgb)
      + sample_tap(i.layer_taps.w, is_side, i.box_uv, ddx, ddy, i.rgb)) * 0.25;
  }
  // The blend: an amount, or 2 + amount for the complementary pattern.
  let inverted = d.visible > 1.5;
  let amount = select(d.visible, d.visible - 2.0, inverted);
  let pattern = dither_threshold(i.clip.xy);
  if (select(pattern >= amount, pattern < amount, inverted)) {
    discard;
  }
  var base = taps;
  base = base * mix(1.0, 1.0 - frame.edge_strength, edge_factor(i.local_pos, i.box_scale, i.normal, i.faces));

  // A box the ground was shrunk into is lit under as it is lit over: what
  // lies below it is the rest of that ground, not a hollow (faces.wgsl).
  let under = i.normal.z < -0.5 && face_bit(i.faces, UNDER_AS_TOP);
  let n = normalize(select(i.normal, vec3f(i.normal.xy, -i.normal.z), under));
  // Darkened by k, and warmed by the same k, so a fully open face is untouched.
  let k = frame.ao_strength * occlusion(i.local_pos, i.box_scale, i.normal, i.faces);
  let open = (1.0 - k) * mix(vec3f(1.0), AO_TINT, k);
  let color = screen_ao(shade(base, n, i.world_pos, sun_lit(i.world_pos, n), open, Seen(i.through, i.scattered, i.past, i.fill)), i.clip.xy);
  return vec4f(finish(color, i.clip.xy), d.opacity);
}
