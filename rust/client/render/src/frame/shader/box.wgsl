// Draws the ghost's box, unfolded from its draw's uniform into a cube's six
// faces, every edge outlined as its silhouette.

// The draws' uniforms by slot (word.rs `ghost_box`), read as whole vec4s. Two
// backends lay out a vec3f followed by scalars in a storage array differently,
// and every member after the first slides.
@group(0) @binding(3) var<storage, read> draws: array<vec4f>;

var<private> draw_slot: u32;

fn ghost_box_word(k: u32) -> vec4f { return draws[draw_slot * DRAW_VEC4S + k]; }

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

// Vertex `k` of the box's 36, two triangles a face wound counter-clockwise seen from outside,
// the faces numbered as paint.wgsl `face_normal` numbers them. The raster's corners
// (`raster_corner`) are the same cube's with the faces the low side first, so a face's number
// there is this one's with its last bit turned.
fn box_corner(k: u32, side: f32, place: vec3f) -> Corner {
  let face = k / 6u;
  let local = raster_corner((face ^ 1u) * 6u + k % 6u) - 0.5;
  let size = vec3f(side);
  return Corner(face, local, size, place + size * (local + 0.5));
}

// The fraction of a unit past the whole unit under the box's corner, in the
// world's units. A unit is `1 << FRAC_W` Q3 steps, so a word's low bits are that
// fraction whichever lap the corner lies on.
fn box_fraction() -> vec3f {
  return vec3f(vec3u(ghost_box_q()) & vec3u((1u << FRAC_W) - 1u)) * Q3_STEP + ghost_box_r();
}

// The draw's uniform slot rides on `vertex_index`: a draw's first vertex is
// its slot times the cube's vertices.
@vertex
fn vs(@builtin(vertex_index) v: u32) -> VertexOut {
  draw_slot = v / BOX_VERTEX_CNT;
  let c = box_corner(v % BOX_VERTEX_CNT, ghost_box_side(), from_eye(ghost_box_q(), ghost_box_r()));
  var o: VertexOut;
  o.clip = jittered(frame_view_proj() * vec4f(c.world, 1.0));
  let local = c.local;
  let size = c.size;
  let normal = face_normal(c.face);
  o.rgb = srgb_to_linear(unpack4x8unorm(ghost_box_rgb()).xyz);
  o.layer = f32(ghost_box_layer());
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
  // the face stands in the world: the corner's fraction past a whole unit,
  // plus the face's way across the box. On a side, v runs down.
  o.box_uv = face_uv(box_fraction() + size * (local + 0.5), normal);
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
