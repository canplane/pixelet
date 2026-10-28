// The ghost's boxes: each unfolded into a cube's six faces, every edge of
// it its own silhouette.

// Two vec4s, not a vec3f followed by scalars: that layout inside a storage
// array is read differently by two backends, everything past the first member sliding.
struct Draw {
  place: vec4f,
  // Opacity, the box's cell in units, the height step, padding.
  measure: vec4f,
};

@group(0) @binding(3) var<storage, read> draws: array<Draw>;
struct BoxWords {
  @location(0) first: vec4<u32>,
  @location(1) layer: u32,
};

// x/y in steps of its own cell, floor and height in steps of the mesh's.
struct Box {
  min_xy: vec2f,
  size_xy: vec2f,
  z: vec2f,
  rgb: u32,
  layer: u32,
};

const XY_STEPS: f32 = f32(BOX_XY_STEPS);

fn box_pair(word: u32, step: f32) -> vec2f {
  return vec2f(f32(word & 0xffffu), f32(word >> 16u)) * step;
}

fn box_of(words: BoxWords, d: Draw) -> Box {
  let step = d.measure.y / XY_STEPS;
  var b: Box;
  b.z = box_pair(words.first[2], d.measure.z);
  b.min_xy = box_pair(words.first[0], step);
  b.size_xy = box_pair(words.first[1], step);
  b.rgb = words.first[3];
  b.layer = words.layer;
  return b;
}

// Wound counter-clockwise from outside, in paint.wgsl's FACE_NORMAL order.
const FACE_CORNER = array<vec3f, 24>(
  vec3f(0.5, -0.5, -0.5), vec3f(0.5, 0.5, -0.5), vec3f(0.5, 0.5, 0.5), vec3f(0.5, -0.5, 0.5),
  vec3f(-0.5, -0.5, -0.5), vec3f(-0.5, -0.5, 0.5), vec3f(-0.5, 0.5, 0.5), vec3f(-0.5, 0.5, -0.5),
  vec3f(-0.5, 0.5, -0.5), vec3f(-0.5, 0.5, 0.5), vec3f(0.5, 0.5, 0.5), vec3f(0.5, 0.5, -0.5),
  vec3f(-0.5, -0.5, -0.5), vec3f(0.5, -0.5, -0.5), vec3f(0.5, -0.5, 0.5), vec3f(-0.5, -0.5, 0.5),
  vec3f(-0.5, -0.5, 0.5), vec3f(0.5, -0.5, 0.5), vec3f(0.5, 0.5, 0.5), vec3f(-0.5, 0.5, 0.5),
  vec3f(-0.5, -0.5, -0.5), vec3f(-0.5, 0.5, -0.5), vec3f(0.5, 0.5, -0.5), vec3f(0.5, -0.5, -0.5),
);
const FACE_TRIANGLES = array<u32, 6>(0u, 1u, 2u, 0u, 2u, 3u);

struct VertexOut {
  @builtin(position) clip: vec4f,
  @location(0) @interpolate(flat) rgb: vec3f,
  @location(1) @interpolate(flat) layer: f32,
  @location(2) box_uv: vec2f,
  @location(3) local_pos: vec3f,
  @location(4) @interpolate(flat) box_scale: vec3f,
  @location(5) @interpolate(flat) normal: vec3f,
  @location(6) world_pos: vec3f,
  @location(7) @interpolate(flat) opacity: f32,
  @location(8) through: f32,
  @location(9) scattered: vec3f,
  @location(10) @interpolate(flat) fill: vec3f,
  @location(11) @interpolate(flat) sun: vec3f,
  @location(12) @interpolate(flat) other: vec3f,
};

// The draw record's index rides on vertex_index (first vertex = record index
// times the cube); the first box is the draw's first instance.
@vertex
fn vs(@builtin(vertex_index) v: u32, words: BoxWords) -> VertexOut {
  return box_vertex(v, words);
}

struct Corner {
  face: u32,
  local: vec3f,
  size: vec3f,
  world: vec3f,
};

fn box_corner(k: u32, b: Box, place: vec3f) -> Corner {
  var c: Corner;
  c.face = k / 6u;
  c.local = FACE_CORNER[c.face * 4u + FACE_TRIANGLES[k % 6u]];
  c.size = vec3f(b.size_xy, b.z.y);
  let position = vec3f(b.min_xy, b.z.x) + c.size * 0.5 + c.local * c.size;
  c.world = position + place;
  return c;
}


fn box_vertex(v: u32, words: BoxWords) -> VertexOut {
  let d = draws[v / BOX_VERTICES];
  let b = box_of(words, d);
  let c = box_corner(v % BOX_VERTICES, b, d.place.xyz);
  var o: VertexOut;
  o.clip = frame_view_proj() * vec4f(c.world, 1.0);
  let local = c.local;
  let size = c.size;
  let normal = FACE_NORMAL[c.face];
  o.rgb = srgb_to_linear(unpack4x8unorm(b.rgb).xyz);
  o.layer = f32(b.layer);
  o.local_pos = local;
  o.box_scale = size;
  o.normal = normal;
  o.opacity = d.measure.x;
  o.world_pos = c.world;
  let seen = seen_from_eye(c.world);
  o.through = seen.through;
  o.scattered = seen.scattered;
  o.sun = seen.light.sun;
  o.other = seen.light.other;
  o.fill = seen.light.fill;
  // Real-size tile UV, one repeat a unit, read from where the face stands and
  // not from its box: a leaf cut into several boxes still wears one tile. The
  // mesh's corner and floor stand on whole units (ghost.rs `on_whole_units`), so its own
  // coordinates are the world's modulo the repeat; v runs down a side.
  o.box_uv = face_uv(vec3f(b.min_xy, b.z.x) + size * (local + 0.5), normal);
  return o;
}

// How much the box's own edges darken a point of its face: a lone box's
// every edge is its silhouette, the nearer of the two edges of the face
// that run through the point's axes. `pixel` is the units one of the
// surface's pixels spans along each axis: an edge is the cells' line, of
// one width in units whatever the box's size (`line_at`).
fn outline(local_pos: vec3f, box_scale: vec3f, pixel: vec3f, normal: vec3f) -> f32 {
  if (frame_edge_strength() <= 0.0) {
    return 0.0;
  }
  let to_edge = (0.5 - abs(local_pos)) * box_scale;
  let p = max(pixel, vec3f(1e-9));
  var a = vec2u(0u, 2u);
  if (abs(normal.z) > 0.5) {
    a = vec2u(0u, 1u);
  } else if (abs(normal.x) > 0.5) {
    a = vec2u(1u, 2u);
  }
  return max(line_at(to_edge[a.x], p[a.x]), line_at(to_edge[a.y], p[a.y]));
}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  let at = i.local_pos * i.box_scale;
  let pixel = sqrt(dpdx(at) * dpdx(at) + dpdy(at) * dpdy(at));
  let s = Surface(
    i.rgb,
    vec4f(i.layer),
    i.normal,
    outline(i.local_pos, i.box_scale, pixel, i.normal),
    i.world_pos,
    i.box_uv,
    dpdx(i.box_uv),
    dpdy(i.box_uv),
    vec3f(screen_open(i.clip.xy)),
    Seen(i.through, i.scattered, Light(i.sun, i.other, i.fill)),
  );
  return vec4f(finish(paint(s), i.clip.xy), i.opacity);
}
