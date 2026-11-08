// Draws the ghost's boxes. Each box is unfolded into a cube's six faces, and
// every edge of a box is outlined as its silhouette.

// The draws' uniforms by slot (word.rs `ghost_box`), read as whole vec4s. Two
// backends lay out a vec3f followed by scalars in a storage array differently,
// and every member after the first slides.
@group(0) @binding(3) var<storage, read> draws: array<vec4f>;

var<private> draw_slot: u32;

fn ghost_box_word(k: u32) -> vec4f { return draws[draw_slot * DRAW_VEC4S + k]; }

struct BoxWords {
  @location(0) first: vec4<u32>,
  @location(1) layer: u32,
};

// A box's x and y are in steps of its own cell, and its floor and height in
// steps of the mesh's.
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

fn box_of(words: BoxWords) -> Box {
  let step = ghost_box_unit() / XY_STEPS;
  return Box(box_pair(words.first[0], step), box_pair(words.first[1], step), box_pair(words.first[2], ghost_box_z_step()), words.first[3], words.layer);
}

// Each face's four corners, wound counter-clockwise seen from outside, with the
// faces in the order of paint.wgsl's `FACE_NORMAL`.
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
  @location(8) transmittance: f32,
  @location(9) scattered: vec3f,
  @location(10) @interpolate(flat) fill: vec3f,
  @location(11) @interpolate(flat) sun: vec3f,
  @location(12) @interpolate(flat) other: vec3f,
};

struct Corner {
  face: u32,
  local: vec3f,
  size: vec3f,
  world: vec3f,
};

fn box_corner(k: u32, b: Box, place: vec3f) -> Corner {
  let face = k / 6u;
  let local = FACE_CORNER[face * 4u + FACE_TRIANGLES[k % 6u]];
  let size = vec3f(b.size_xy, b.z.y);
  let pos = vec3f(b.min_xy, b.z.x) + size * 0.5 + local * size;
  return Corner(face, local, size, pos + place);
}


// The draw record's index rides on `vertex_index`: a draw's first vertex is
// its record's index times the cube's vertices. The draw's first instance is
// its first box.
@vertex
fn vs(@builtin(vertex_index) v: u32, words: BoxWords) -> VertexOut {
  draw_slot = v / BOX_VERTICES;
  let b = box_of(words);
  let c = box_corner(v % BOX_VERTICES, b, from_eye(ghost_box_q(), ghost_box_r()));
  var o: VertexOut;
  o.clip = jittered(frame_view_proj() * vec4f(c.world, 1.0));
  let local = c.local;
  let size = c.size;
  let normal = FACE_NORMAL[c.face];
  o.rgb = srgb_to_linear(unpack4x8unorm(b.rgb).xyz);
  o.layer = f32(b.layer);
  o.local_pos = local;
  o.box_scale = size;
  o.normal = normal;
  o.opacity = ghost_box_opacity();
  o.world_pos = c.world;
  let seen = seen_from_eye(c.world);
  o.transmittance = seen.transmittance;
  o.scattered = seen.scattered;
  let l = light_of(c.world);
  o.sun = l.sun;
  o.other = l.other;
  o.fill = l.fill;
  // The tile's UV is at real size, one repeat a unit, and is read from where
  // the face stands, not from its box, so a leaf cut into several boxes still
  // wears one tile. The mesh's corner and floor stand on whole units (ghost.rs
  // `on_whole_units`), so the mesh's own coordinates are the world's modulo
  // the repeat. On a side, v runs down.
  o.box_uv = face_uv(vec3f(b.min_xy, b.z.x) + size * (local + 0.5), normal);
  return o;
}

// How much the box's own edges darken a point of its face. Every edge of a
// lone box is its silhouette, and the point takes the nearer of the face's
// two edges along its axes. `px` is how many units one of the surface's
// pixels spans along each axis. An edge is drawn as the cells' line is, of
// one width in units whatever the box's size (`line_at`).
fn outline(local_pos: vec3f, box_scale: vec3f, px: vec3f, normal: vec3f) -> f32 {
  if (frame_edge_strength() <= 0.0) {
    return 0.0;
  }
  let to_edge = (0.5 - abs(local_pos)) * box_scale;
  let p = max(px, vec3f(1e-9));
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
  let px = sqrt(dpdx(at) * dpdx(at) + dpdy(at) * dpdy(at));
  let s = Surface(
    i.rgb,
    i.layer,
    i.normal,
    outline(i.local_pos, i.box_scale, px, i.normal),
    i.world_pos,
    i.box_uv,
    dpdx(i.box_uv),
    dpdy(i.box_uv),
    1.0,
  );
  return vec4f(tone(paint(s, Air(i.transmittance, i.scattered), Light(i.sun, i.other, i.fill))), i.opacity);
}
