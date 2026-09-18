// The chunk mesh: the mesher's lanes as they lie in a block, one vertex
// slot a lane. Every rule here was chunk_material.ts's injected GLSL over
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
  _p0: f32,
  _p1: f32,
  _p2: f32,
};

@group(0) @binding(3) var<storage, read> draws: array<Draw>;
@group(0) @binding(4) var layers_top: texture_2d_array<f32>;
@group(0) @binding(5) var layers_side: texture_2d_array<f32>;
@group(0) @binding(6) var layer_sampler: sampler;

// volume.rs's NO_LAYER: a tap past the last layer means the vertex colour.
const NO_LAYER: f32 = 255.0;

struct VertexIn {
  @builtin(instance_index) draw: u32,
  @location(0) position: vec3f,
  @location(1) normal: vec3f,
  @location(2) local_pos: vec3f,
  @location(3) box_scale: vec3f,
  @location(4) rgb: vec3f,
  @location(5) layer_taps: vec4f,
  @location(6) faces: f32,
};

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
};

@vertex
fn vs(v: VertexIn) -> VertexOut {
  let d = draws[v.draw];
  let world = v.position + d.translation;
  var o: VertexOut;
  o.clip = frame.view_proj * vec4f(world, 1.0);
  o.rgb = v.rgb;
  o.layer_taps = v.layer_taps;
  o.local_pos = v.local_pos;
  o.box_scale = v.box_scale;
  o.normal = v.normal;
  o.faces = u32(v.faces + 0.5);
  o.draw = v.draw;
  o.world_pos = world;
  // Real-size tile UV: a box's own [-0.5, 0.5] corner times its real size
  // per axis, so a box spanning many cells repeats the tile once a cell.
  // Side faces put height in v with the box's top at v = 0, the image's
  // top row; the array is sampled unflipped.
  let is_side = abs(v.normal.z) < 0.5;
  if (is_side) {
    if (abs(v.normal.x) > 0.5) {
      o.box_uv = vec2f((v.local_pos.y + 0.5) * v.box_scale.y, (0.5 - v.local_pos.z) * v.box_scale.z);
    } else {
      o.box_uv = vec2f((v.local_pos.x + 0.5) * v.box_scale.x, (0.5 - v.local_pos.z) * v.box_scale.z);
    }
  } else {
    o.box_uv = vec2f((v.local_pos.x + 0.5) * v.box_scale.x, (v.local_pos.y + 0.5) * v.box_scale.y);
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

// Both arrays are sampled and one chosen: textureSample must sit in
// uniform control flow, and which face this is, is not.
fn sample_tap(layer: f32, is_side: bool, uv: vec2f, fallback: vec3f) -> vec3f {
  let l = i32(clamp(layer, 0.0, 255.0) + 0.5);
  let top = textureSample(layers_top, layer_sampler, uv, l).rgb;
  let side = textureSample(layers_side, layer_sampler, uv, l).rgb;
  let tex = select(top, side, is_side);
  return select(tex, fallback, layer > NO_LAYER - 0.5);
}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  let d = draws[i.draw];
  let is_side = abs(i.normal.z) < 0.5;
  let taps = (sample_tap(i.layer_taps.x, is_side, i.box_uv, i.rgb)
    + sample_tap(i.layer_taps.y, is_side, i.box_uv, i.rgb)
    + sample_tap(i.layer_taps.z, is_side, i.box_uv, i.rgb)
    + sample_tap(i.layer_taps.w, is_side, i.box_uv, i.rgb)) * 0.25;
  // The blend: an amount, or 2 + amount for the complementary pattern.
  let inverted = d.visible > 1.5;
  let amount = select(d.visible, d.visible - 2.0, inverted);
  let pattern = dither_threshold(i.clip.xy);
  if (select(pattern >= amount, pattern < amount, inverted)) {
    discard;
  }
  var base = select(taps, i.rgb, frame.low_spec > 0.5);
  base = base * mix(1.0, 1.0 - frame.edge_strength, edge_factor(i.local_pos, i.box_scale, i.normal, i.faces));

  let n = normalize(i.normal);
  let color = shade(base, n, i.world_pos, 1.0 / i.clip.w, sun_lit(i.world_pos, n));
  return vec4f(finish(color, i.clip.xy), d.opacity);
}
