// pxl-render/frame/shader/screen_cells.wgsl

// The rays' cells (`frame/screen_cells.rs`), each drawn straight into its own
// area of the screen: the lane's cells across and up, at equal steps of angle
// over the view, each pixel the face its cell's ray entered, where the
// pixel's own ray meets that face, painted as a chunk paints it
// (paint.wgsl `paint`). A cell whose ray met nothing is left to the sky.

// The lane (`frame/screen_cells.rs`): the cells across and up and whether a
// cell is coloured by its ray's steps, then a cell each, what its ray met
// (value, kind, face, zoom), how far it went (f32 bits), its face bits, and
// its fold's looks or, in the steps view, the nodes its descent asked.
@group(0) @binding(3) var<storage, read> cells: array<vec4u>;
// Each look's swatch and layer, then the colour it reads as from afar
// (`mesh/paint/ground.rs` `look_words`): the ground's looks, then each
// placed value's.
@group(0) @binding(20) var<storage, read> looks: array<vec2u>;
// Where the eye is in the world: its whole units, wrapping, then what is
// left of a unit, f32 bits; a block's place on the lattice is read off it.
@group(0) @binding(21) var<uniform> eye_at: array<vec4u, 2>;

const KIND_GROUND: u32 = 1u;
const KIND_SHIFT: u32 = 16u;
const FACE_SHIFT: u32 = 18u;
const ZOOM_SHIFT: u32 = 21u;
const GROUND_LOOKS: u32 = 256u;
// The steps a ray's colour runs up to, on a log scale, the same on every
// screen so two compare.
const STEPS_MOST: f32 = 1024.0;
// Where a ray that met nothing is put when it is coloured by its steps.
const FAR: f32 = 1.0e6;

const FACE_NORMAL = array<vec3f, 6>(vec3f(1.0, 0.0, 0.0), vec3f(-1.0, 0.0, 0.0), vec3f(0.0, 1.0, 0.0), vec3f(0.0, -1.0, 0.0), vec3f(0.0, 0.0, 1.0), vec3f(0.0, 0.0, -1.0));

struct VertexOut {
  @builtin(position) clip: vec4f,
  @location(0) ndc: vec2f,
};

struct FragOut {
  @location(0) color: vec4f,
  @builtin(frag_depth) depth: f32,
};

@vertex
fn vs(@builtin(vertex_index) i: u32) -> VertexOut {
  let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u)) * 2.0 - 1.0;
  var o: VertexOut;
  o.clip = vec4f(p, 0.0, 1.0);
  o.ndc = p;
  return o;
}

// Google's Turbo colour map, its polynomial fit: dark blue at 0, through
// green and yellow, dark red at 1.
fn turbo(x: f32) -> vec3f {
  let v = vec4f(1.0, x, x * x, x * x * x);
  let v2 = v.zw * v.z;
  let r = dot(v, vec4f(0.13572138, 4.61539260, -42.66032258, 132.13108234)) + dot(v2, vec2f(-152.94239396, 59.28637943));
  let g = dot(v, vec4f(0.09140261, 2.19418839, 4.84296658, -14.18503333)) + dot(v2, vec2f(4.27729857, 2.82956604));
  let b = dot(v, vec4f(0.10667330, 12.64194608, -60.58204836, 110.36276771)) + dot(v2, vec2f(-89.90310912, 27.34824973));
  return clamp(vec3f(r, g, b), vec3f(0.0), vec3f(1.0));
}

fn srgb_to_linear(c: vec3f) -> vec3f {
  let low = c / 12.92;
  let high = pow((c + 0.055) / 1.055, vec3f(2.4));
  return select(high, low, c <= vec3f(0.04045));
}

// A look's words; past the table, black and no layer.
fn look(slot: u32) -> vec2u {
  if (slot < arrayLength(&looks)) {
    return looks[slot];
  }
  return vec2u(u32(NO_LAYER) << 24u, 0u);
}

fn linear_of(word: u32) -> vec3f {
  return srgb_to_linear(unpack4x8unorm(word).rgb);
}

// What a look reads as on a block coarser than the ground's cell: its
// layer's mean where the pack is drawn, else its swatch.
fn far(l: vec2u) -> vec3f {
  return linear_of(select(l.x, l.y, frame_low_spec() < 0.5));
}

struct Drawn {
  rgb: vec3f,
  taps: vec4f,
};

// What the block a cell's ray met is drawn as: at the ground's cell or finer, its swatch and
// its layer in all four taps; coarser, one colour, a ground fold's the mean
// of the looks it folds where its face's own is among them.
fn drawn(c: vec4u) -> Drawn {
  let value = c.x & 0xffffu;
  let ground = ((c.x >> KIND_SHIFT) & 3u) == KIND_GROUND;
  let l = look(select(GROUND_LOOKS + value, value, ground));
  if (((c.x >> ZOOM_SHIFT) & 63u) >= GROUND_CELL) {
    return Drawn(linear_of(l.x), vec4f(f32(l.x >> 24u)));
  }
  let fold = vec4u(c.w & 0xffu, (c.w >> 8u) & 0xffu, (c.w >> 16u) & 0xffu, c.w >> 24u);
  if (ground && any(fold == vec4u(value))) {
    let sum = far(look(fold.x)) + far(look(fold.y)) + far(look(fold.z)) + far(look(fold.w));
    return Drawn(sum * 0.25, vec4f(NO_LAYER));
  }
  return Drawn(far(l), vec4f(NO_LAYER));
}

// Along `axis`, how far the eye is past the start of the block of side
// 2^k units it stands in.
fn eye_phase(axis: u32, k: i32) -> f32 {
  let part = bitcast<f32>(eye_at[1][axis]);
  if (k <= 0) {
    let side = exp2(f32(k));
    return part - floor(part / side) * side;
  }
  return f32(eye_at[0][axis] & ((1u << u32(k)) - 1u)) + part;
}

// Where a pixel's ray meets the face its cell's ray entered, from the eye,
// kept on the block: the block, half its side in from where the cell's ray
// entered it, its corner on the lattice, and the face's plane through it.
struct Hit {
  at: vec3f,
  corner: vec3f,
  side: f32,
  normal: vec3f,
};

fn hit_of(c: vec4u, ray: vec3f, d: vec3f) -> Hit {
  let face = min((c.x >> FACE_SHIFT) & 7u, 5u);
  let axis = face / 2u;
  let n = FACE_NORMAL[face];
  let k = i32(UNIT_ZOOM) - i32((c.x >> ZOOM_SHIFT) & 63u);
  let side = exp2(f32(k));
  let e = vec3f(eye_phase(0u, k), eye_phase(1u, k), eye_phase(2u, k));
  let t = bitcast<f32>(c.y);
  let corner = floor((e + ray * t - n * (0.5 * side)) / side) * side - e;
  let plane = corner[axis] + select(0.0, side, n[axis] > 0.0);
  let s = select(t, plane / d[axis], d[axis] * plane > 0.0);
  return Hit(clamp(d * s, corner, corner + vec3f(side)), corner, side, n);
}

// The face's texture coordinate: the world's position, one repeat a unit,
// as a chunk's mesh reads it off its own whole-unit coordinates.
fn box_uv(h: Hit) -> vec2f {
  let p = bitcast<vec3f>(eye_at[1].xyz) + h.at;
  if (abs(h.normal.z) > 0.5) {
    return p.xy;
  }
  return select(vec2f(p.x, -p.z), vec2f(p.y, -p.z), abs(h.normal.x) > 0.5);
}

@fragment
fn fs(v: VertexOut) -> FragOut {
  let m = sky_ray();
  let dir = m * vec3f(v.ndc, 1.0);
  let forward = length(m[2]);
  let axes = mat3x3f(normalize(m[0]), normalize(m[1]), m[2] / forward);
  let z = dot(dir, axes[2]);
  // Each axis's angle from the view's edge, counted in cells.
  let angle = vec2f(atan2(dot(dir, axes[0]), z), atan2(dot(dir, axes[1]), z));
  let half = vec2f(atan2(length(m[0]), forward), atan2(length(m[1]), forward));
  let across = cells[0].xy;
  let at = vec2i(floor((angle + half) / (2.0 * half) * vec2f(across)));
  if (any(at < vec2i(0)) || any(vec2u(at) >= across)) {
    discard;
  }
  let c = cells[1u + u32(at.y) * across.x + u32(at.x)];
  let kind = (c.x >> KIND_SHIFT) & 3u;
  // The cell's own ray, through its middle, as the cast formed it.
  let mid = ((vec2f(at) + 0.5) / vec2f(across) * 2.0 - 1.0) * half;
  let ray = normalize(axes * vec3f(tan(mid), 1.0));
  let d = normalize(dir);
  let h = hit_of(c, ray, d);
  // Taken before any branch: a derivative needs the quad whole.
  let uv = box_uv(h);
  let ddx = dpdx(uv);
  let ddy = dpdy(uv);
  let pixel = sqrt(dpdx(h.at) * dpdx(h.at) + dpdy(h.at) * dpdy(h.at));
  if (cells[0].z != 0u) {
    let t = select(bitcast<f32>(c.y), FAR, kind == 0u);
    let clip = frame_view_proj() * vec4f(d * t, 1.0);
    var o: FragOut;
    o.color = vec4f(turbo(log2(1.0 + f32(c.w)) / log2(1.0 + STEPS_MOST)), 1.0);
    o.depth = clip.z / clip.w;
    return o;
  }
  if (kind == 0u) {
    discard;
  }
  let world = on_globe(h.at);
  let painted = drawn(c);
  let s = Surface(
    painted.rgb,
    painted.taps,
    h.normal,
    (h.at - h.corner) / h.side - 0.5,
    vec3f(h.side),
    h.side,
    c.z,
    world,
    uv,
    ddx,
    ddy,
    pixel,
    screen_open(v.clip.xy),
    seen_from_eye(world),
  );
  let clip = frame_view_proj() * vec4f(world, 1.0);
  var o: FragOut;
  o.color = vec4f(finish(paint(s), v.clip.xy), 1.0);
  o.depth = clip.z / clip.w;
  return o;
}
