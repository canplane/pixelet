// The chunk mesh: each box unfolded into a cube's six faces, a face the mask
// lacks collapsed to nothing. No tone mapping, as three applied none here.

// Two vec4s, not a vec3f followed by scalars: that layout inside a storage
// array is read differently by two backends, everything past the first member sliding.
struct Draw {
  place: vec4f,
  // Opacity, the box's cell in units, the height step, padding.
  measure: vec4f,
};

@group(0) @binding(3) var<storage, read> draws: array<Draw>;
// The last two words exist only in the fine format; a coarse draw binds the
// same buffer for them so one shader serves both.
struct BoxWords {
  @location(0) first: vec4<u32>,
  @location(1) rest: vec2<u32>,
};

// x/y in steps of its own cell, floor and height in steps of the mesh's.
struct Box {
  min_xy: vec2f,
  size_xy: vec2f,
  z: vec2f,
  rgb: u32,
  taps: u32,
  faces: u32,
};

const XY_STEPS: f32 = 256.0;
const COARSE_XY_STEPS: f32 = 8.0;
const NO_TAPS: u32 = 0xffffffffu;

fn box_pair(word: u32, step: f32) -> vec2f {
  return vec2f(f32(word & 0xffffu), f32(word >> 16u)) * step;
}

fn box_of(words: BoxWords, d: Draw, fine: bool) -> Box {
  let unit = d.measure.y;
  var b: Box;
  b.z = box_pair(words.first[2], d.measure.z);
  if (fine) {
    let step = unit / XY_STEPS;
    b.min_xy = box_pair(words.first[0], step);
    b.size_xy = box_pair(words.first[1], step);
    b.rgb = words.first[3];
    b.taps = words.rest[0];
    b.faces = words.rest[1];
    return b;
  }
  let step = unit / COARSE_XY_STEPS;
  b.faces = words.first[0];
  b.rgb = words.first[1];
  let w = words.first[3];
  b.min_xy = vec2f(f32(w & 0xffu), f32((w >> 8u) & 0xffu)) * step;
  // A width is held one short, so a whole mesh across fits in a byte.
  b.size_xy = (vec2f(f32((w >> 16u) & 0xffu), f32((w >> 24u) & 0xffu)) + 1.0) * step;
  b.taps = NO_TAPS;
  return b;
}

// Wound counter-clockwise from outside, in the order of the mask's face bits.
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
const FACE_TRIANGLES = array<u32, 6>(0u, 1u, 2u, 0u, 2u, 3u);
const BOX_VERTICES: u32 = 36u;

fn srgb_to_linear(c: vec3f) -> vec3f {
  let low = c / 12.92;
  let high = pow((c + 0.055) / 1.055, vec3f(2.4));
  return select(high, low, c <= vec3f(0.04045));
}

struct VertexOut {
  @builtin(position) clip: vec4f,
  @location(0) @interpolate(flat) rgb: vec3f,
  @location(1) @interpolate(flat) layer_taps: vec4f,
  @location(2) box_uv: vec2f,
  @location(3) local_pos: vec3f,
  @location(4) @interpolate(flat) box_scale: vec3f,
  @location(5) @interpolate(flat) normal: vec3f,
  @location(6) world_pos: vec3f,
  @location(7) @interpolate(flat) faces: u32,
  @location(8) @interpolate(flat) opacity: f32,
  @location(9) through: f32,
  @location(10) scattered: vec3f,
  @location(11) @interpolate(flat) fill: vec3f,
  @location(12) @interpolate(flat) sun: vec3f,
  @location(13) @interpolate(flat) other: vec3f,
  @location(14) @interpolate(flat) cell: f32,
};

// The draw record's index rides on vertex_index (first vertex = record index
// times the cube); the first box is the draw's first instance.
@vertex
fn vs(@builtin(vertex_index) v: u32, words: BoxWords) -> VertexOut {
  return chunk_vertex(v, words, true);
}

// The coarse format: the attribute stride is fixed per pipeline, so it is an
// entry point rather than a per-draw number.
@vertex
fn vs_coarse(@builtin(vertex_index) v: u32, words: BoxWords) -> VertexOut {
  return chunk_vertex(v, words, false);
}

@vertex
fn vs_depth(@builtin(vertex_index) v: u32, words: BoxWords) -> @builtin(position) vec4f {
  return chunk_depth(v, words, true);
}

@vertex
fn vs_coarse_depth(@builtin(vertex_index) v: u32, words: BoxWords) -> @builtin(position) vec4f {
  return chunk_depth(v, words, false);
}

struct Corner {
  face: u32,
  shown: bool,
  local: vec3f,
  size: vec3f,
  world: vec3f,
};

fn box_corner(k: u32, b: Box, place: vec3f) -> Corner {
  var c: Corner;
  c.face = k / 6u;
  c.shown = (b.faces & (1u << c.face)) != 0u;
  if (!c.shown) {
    return c;
  }
  c.local = FACE_CORNER[c.face * 4u + FACE_TRIANGLES[k % 6u]];
  c.size = vec3f(b.size_xy, b.z.y);
  let position = vec3f(b.min_xy, b.z.x) + c.size * 0.5 + c.local * c.size;
  c.world = on_globe(position + place);
  return c;
}

// A masked face collapses to one point, a zero-area triangle the rasterizer drops.
fn corner_clip(c: Corner) -> vec4f {
  if (!c.shown) {
    return vec4f(0.0, 0.0, 0.0, 1.0);
  }
  return frame_view_proj() * vec4f(c.world, 1.0);
}

fn chunk_depth(v: u32, words: BoxWords, fine: bool) -> vec4f {
  let d = draws[v / BOX_VERTICES];
  return corner_clip(box_corner(v % BOX_VERTICES, box_of(words, d, fine), d.place.xyz));
}

fn chunk_vertex(v: u32, words: BoxWords, fine: bool) -> VertexOut {
  let d = draws[v / BOX_VERTICES];
  let b = box_of(words, d, fine);
  let c = box_corner(v % BOX_VERTICES, b, d.place.xyz);
  var o: VertexOut;
  o.clip = corner_clip(c);
  if (!c.shown) {
    return o;
  }
  let local = c.local;
  let size = c.size;
  let normal = FACE_NORMAL[c.face];
  o.rgb = srgb_to_linear(unpack4x8unorm(b.rgb).xyz);
  o.layer_taps = unpack4x8unorm(b.taps) * 255.0;
  o.local_pos = local;
  o.box_scale = size;
  o.normal = normal;
  o.faces = b.faces;
  o.opacity = d.measure.x;
  o.cell = d.measure.y;
  o.world_pos = c.world;
  let seen = seen_from_eye(c.world);
  o.through = seen.through;
  o.scattered = seen.scattered;
  o.sun = seen.light.sun;
  o.other = seen.light.other;
  o.fill = seen.light.fill;
  // Real-size tile UV, one repeat a unit, read from where the face stands and
  // not from its box: a leaf cut into several boxes still wears one tile. The
  // mesh's corner and floor stand on whole units (Heights::of), so its own
  // coordinates are the world's modulo the repeat; v runs down a side.
  let p = vec3f(b.min_xy, b.z.x) + size * (local + 0.5);
  let is_side = abs(normal.z) < 0.5;
  if (is_side) {
    if (abs(normal.x) > 0.5) {
      o.box_uv = vec2f(p.y, -p.z);
    } else {
      o.box_uv = vec2f(p.x, -p.z);
    }
  } else {
    o.box_uv = p.xy;
  }
  return o;
}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  let at = i.local_pos * i.box_scale;
  let s = Surface(
    i.rgb,
    i.layer_taps,
    i.normal,
    i.local_pos,
    i.box_scale,
    i.cell,
    i.faces,
    i.world_pos,
    i.box_uv,
    dpdx(i.box_uv),
    dpdy(i.box_uv),
    sqrt(dpdx(at) * dpdx(at) + dpdy(at) * dpdy(at)),
    screen_open(i.clip.xy),
    Seen(i.through, i.scattered, Light(i.sun, i.other, i.fill)),
  );
  return vec4f(finish(cell_share(paint(s), i), i.clip.xy), i.opacity);
}

// Under the camera gizmo: how many zooms the face's cell is off the one the
// camera grid's rule draws at its distance (`pxl-spec space/camera/grid.rs`
// `cell_at`): the finest whose side covers the grid's cell angle there
// (`frame_cell`), so a side of one to two cells' widths, as `cell_at`'s
// floor of a log2 has it. At the rule's zoom green, finer blue, coarser red.
fn cell_share(color: vec3f, i: VertexOut) -> vec3f {
  if (frame_gizmo() < 0.5) {
    return color;
  }
  let off = floor(log2(i.cell / (length(i.world_pos) * frame_cell())));
  let tint = select(select(vec3f(0.1, 0.9, 0.2), vec3f(0.1, 0.3, 1.0), off < 0.0), vec3f(1.0, 0.15, 0.1), off > 0.0);
  return mix(color, tint * 0.6, 0.45);
}
