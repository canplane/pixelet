// pxl-render/frame/shader/screen_ray.wgsl

// The eye's rays (pxl-spec-core `ray`), a drawn px each: each px
// the face its ray entered, painted by paint.wgsl's `paint`, as the ghost's
// boxes are. A px whose ray met nothing is left to the sky. What lies
// across each edge and corner of a face — the air, the next cell of a flat
// run, a cell rising in front of it — is the world's own answer, asked once
// where the cast stopped (pxl-spec-core `face`) and handed in the px's
// own words: a line is drawn along an edge that meets the air, and the face
// closes in towards an edge or a corner a cell rises across (`creases`). No
// other px's ray is read, so every px of a cell is painted by the
// same bits.

// The rays, a layer a word: what a ray met (val, face, zoom), how far it
// went (f32 bits), the four looks the ground's cell is drawn by or, in the
// steps view, the nodes its descent asked, and its face's edges and corners
// (`RAY_FACES`). A cell is painted from these, its own, alone.
@group(0) @binding(3) var rays: texture_2d_array<u32>;
// Each val's swatch and layer, then the color it reads as from afar, in
// linear light (`mesh/paint/ground.rs` `look_words`), by val whichever
// source holds it.
@group(0) @binding(20) var<storage, read> looks: array<vec4u>;
// The eye the rays left (pxl-spec-core `eye`): its Q3 steps (`EYE_ORIGIN`),
// FRAC_W bits a unit and wrapping, a cell's place on the lattice read off
// their low bits; whether a ray reports its steps (`EYE_STEPS`); its reach;
// the axes and tangents each ray's direction is formed of; and where each
// pixel's ray goes through it (`EYE_JITTER`).
@group(0) @binding(21) var<uniform> eye: array<vec4u, EYE_READ_VEC4S>;

fn eye_word(k: u32) -> u32 {
  return eye[k / 4u][k % 4u];
}

fn eye_vec3(k: u32) -> vec3f {
  return vec3f(bitcast<f32>(eye_word(k)), bitcast<f32>(eye_word(k + 1u)), bitcast<f32>(eye_word(k + 2u)));
}

// The ray along a px's aim (`aim`), as the cast formed it
// (pxl-shader `eye_ray`): pxl-spec-core cast.rs `through`'s mirror,
// off the eye's own words, so the t a ray left is laid along the very
// direction it was cast in.
fn through(a: vec3f) -> vec3f {
  return a / sqrt(dot(a, a));
}

fn eye_tan() -> vec2f {
  return vec2f(bitcast<f32>(eye_word(EYE_TAN)), bitcast<f32>(eye_word(EYE_TAN + 1u)));
}

// Where this frame's rays go through their pixels (pxl-spec-core
// `eye::JITTER`), off their middles, in drawn pixels.
fn eye_jitter() -> vec2f {
  return vec2f(bitcast<f32>(eye_word(EYE_JITTER)), bitcast<f32>(eye_word(EYE_JITTER + 1u)));
}

// That ray's direction before it is made a unit: linear in `frag`.
fn aim(frag: vec2f, size: vec2f) -> vec3f {
  let t = pixel_tan(frag + eye_jitter(), size, eye_tan());
  return eye_vec3(EYE_FORWARD) + t.x * eye_vec3(EYE_RIGHT) + t.y * eye_vec3(EYE_UP);
}

// How far behind its face a ray's depth is written (`behind`): the world is
// the one that steps back, and what is laid on its faces — the puck's zone,
// the ghosts, the markers' anchors — is rasterized with no offset at all. A
// raster on the face rounds apart from the ray's exact depth two ways:
// - its depth interpolated across its corners in f32, about 2^-23 of it:
//   `BEHIND` of the depth, 2^-14, stands past that;
// - its corners snapped to the subpixel grid, at the coarsest a sixteenth
//   of a px (Vulkan's least `subPixelPrecisionBits`, 4): half that on
//   each axis moves its plane at a px's middle by at most a sixteenth of
//   its depth's slope across a px, the most of dx's and dy's as a
//   rasterizer's polygon offset reads it. `SNAP` of that slope, a quarter,
//   stands past it fourfold.
const BEHIND: f32 = 6.103515625e-5;
const SNAP: f32 = 0.25;

struct VertexOut {
  @builtin(position) clip: vec4f,
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
  return o;
}

// A look's words; past the table, black and no layer.
fn look(slot: u32) -> vec4u {
  if (slot < arrayLength(&looks)) {
    return looks[slot];
  }
  return vec4u(0u, u32(NO_LAYER) << 16u, 0u, 0u);
}

fn swatch(l: vec4u) -> vec3f {
  return vec3f(unpack2x16unorm(l.x), unpack2x16unorm(l.y).x);
}

// What a look reads as on a cell coarser than the ground's: its
// layer's mean where the pack is drawn, else its swatch.
fn far(l: vec4u) -> vec3f {
  return select(swatch(l), vec3f(unpack2x16unorm(l.z), unpack2x16unorm(l.w).x), frame_low_spec() < 0.5);
}

struct Drawn {
  rgb: vec3f,
  taps: vec4f,
};

// What the cell a px's ray met is drawn as, by its val, whichever
// source holds it: at the ground's cell or finer, its swatch and its layer
// in all four taps; coarser, one color as it reads from afar, the ground's
// the mean of the four looks its cell folds where its face's own is among
// them (doc/minutes/archive/20260928-what-is-drawn-is-what-the-screen-covers.md,
// "the 2 × 2 is by design").
fn drawn(c: vec3u) -> Drawn {
  let val = ray_val(c.x);
  let l = look(val);
  if (ray_zoom(c.x) >= GROUND_CELL) {
    return Drawn(swatch(l), vec4f(f32(l.y >> 16u)));
  }
  let fold = ray_looks(c.z);
  if (any(fold == vec4u(val))) {
    let sum = far(look(fold.x)) + far(look(fold.y)) + far(look(fold.z)) + far(look(fold.w));
    return Drawn(sum * 0.25, vec4f(NO_LAYER));
  }
  return Drawn(far(l), vec4f(NO_LAYER));
}

// Along `axis`, how far the eye is past the start of the cell of side
// 2^k units it stands in: its steps' low k + FRAC_W bits, every bit at the
// world's side.
fn eye_phase(axis: u32, k: i32) -> f32 {
  let bits = u32(k + i32(FRAC_W));
  let steps = eye_word(EYE_ORIGIN + axis);
  let low = select(steps, steps & ((1u << bits) - 1u), bits < 32u);
  return f32(low) / f32(1u << FRAC_W);
}

// The box a cell keeps of its node `side` a side, `k` its kept eighths (0
// for all eight) and `touch` the faces it touches, a bit a face, 2a the
// low on axis a and 2a + 1 the high: its least corner off the node's and
// its size, as pxl-spec-core cast.rs `kept_bounds` places it. An axis both
// of whose faces touch is spanned, one touching face is sat flush against,
// none is centered, and the `a` axes not spanned are each f^(1/(3 − a))
// of the node, so the volume is the share `f`.
struct Kept {
  lo: vec3f,
  size: vec3f,
};

fn kept_box(k: u32, touch: u32, side: f32) -> Kept {
  let low = vec3<bool>((touch & 1u) != 0u, (touch & 4u) != 0u, (touch & 16u) != 0u);
  let high = vec3<bool>((touch & 2u) != 0u, (touch & 8u) != 0u, (touch & 32u) != 0u);
  let span = low & high;
  let spanned = u32(span.x) + u32(span.y) + u32(span.z);
  if (k == 0u || spanned == 3u) {
    return Kept(vec3f(0.0), vec3f(side));
  }
  let f = f32(k) / f32(KEPT_EIGHTHS);
  let s = select(select(f, sqrt(f), spanned == 1u), kept_side(k), spanned == 0u);
  let size = select(vec3f(side * s), vec3f(side), span);
  let lo = select(select(vec3f(0.5) * (vec3f(side) - size), vec3f(side) - size, high), vec3f(0.0), low);
  return Kept(lo, size);
}

// Where a ray meets the face it entered, from the eye, kept on the cell
// (the cell half the box in from where the ray entered it, its corner on
// the lattice, and the face's plane through it), the face's normal and
// the axis it lies across, the box's corner, its least on every axis, and
// its size, in units: its node's where it keeps its cube, else the box it
// keeps (`kept_box`, by its eighths and the faces it touches).
struct Hit {
  at: vec3f,
  normal: vec3f,
  axis: u32,
  corner: vec3f,
  size: vec3f,
};

fn hit_of(c: vec2u, faces: u32, d: vec3f) -> Hit {
  let face = min(ray_face(c.x), 5u);
  let axis = face / 2u;
  let n = FACE_NORMAL[face];
  let k = i32(UNIT_ZOOM) - i32(ray_zoom(c.x));
  let side = exp2(f32(k));
  let e = vec3f(eye_phase(0u, k), eye_phase(1u, k), eye_phase(2u, k));
  let t = bitcast<f32>(c.y);
  let kept = kept_box(ray_kept(c.x), faces_touch(faces), side);
  let node = floor((e + d * t - n * (0.5 * kept.size[axis])) / side) * side - e;
  let corner = node + kept.lo;
  let plane = corner[axis] + select(0.0, kept.size[axis], n[axis] > 0.0);
  let s = select(t, plane / d[axis], d[axis] * plane > 0.0);
  return Hit(clamp(d * s, corner, corner + kept.size), n, axis, corner, kept.size);
}

// How far this px's ray's hit moves along its face's plane as the px
// moves one drawn px across (the first) and down (the second): what a
// rasterized face's dpdx and dpdy of its pos read. Derived rather than
// differenced, since the quad about an edge straddles other faces there.
fn px_steps(a: vec3f, size: vec2f, h: Hit) -> mat2x3f {
  let tan = eye_tan();
  let ax = eye_vec3(EYE_RIGHT) * (2.0 * tan.x / size.x);
  let ay = eye_vec3(EYE_UP) * (-2.0 * tan.y / size.y);
  let an = dot(a, h.normal);
  let s = dot(h.at, h.normal) / an;
  return mat2x3f(s * (ax - a * (dot(ax, h.normal) / an)), s * (ay - a * (dot(ay, h.normal) / an)));
}

// The units one drawn px spans along each axis where this px's ray
// meets its face's plane.
fn footprint(p: mat2x3f) -> vec3f {
  return sqrt(p[0] * p[0] + p[1] * p[1]);
}

// The share of its depth this px's ray's depth is written behind its
// face: `BEHIND`, and `SNAP` of the face's depth's own slope across a px.
// Depth is `near / w`, `w` the distance along the eye's forward `F`, so a
// share of the depth is the same share of `w`, and the hit's `w` moves by
// `p·F` across a px `p`. With the aim `a` (`a·F = 1`, the hit `w·a`) and
// `a_x` its change across a px, `p_x·F / w = |a_x·n| / |a·n|`.
//
// It never lets an overlay a cell or more behind the face win over it: the
// step is `w·|a|` times the share along the ray, while the face's plane a
// cell `side` back lies `side·|a| / |a·n|` further along it, so the step
// reaches it only once `SNAP·w·|a_x·n| ≥ side`. But `w·|a_x|` is a px's
// width at the hit's distance, a cell of the camera grid there over the
// cosine of the ray off the forward axis, and a cell observed there spans
// at least that cell (the eye decides): the step is at most `SNAP / cos` of
// a cell, under one for any field of view short of 150°, however edge on
// the face is seen. `BEHIND` adds 2^-14 of the distance, where a cell is at
// least 1/96 of it (pxl-spec-core `grid::CELL`).
fn behind(h: Hit, p: mat2x3f) -> f32 {
  let f = eye_vec3(EYE_FORWARD);
  return BEHIND + SNAP * max(abs(dot(p[0], f)), abs(dot(p[1], f))) / dot(h.at, f);
}

// How much a rising edge closes a face `d` cells from it: over one cell
// and no further. Squared, not smoothstep: a smoothstep sits flat against
// the wall and reads as a dark strip with an edge rather than a gradient.
fn occlusion_of(d: f32) -> f32 {
  let t = 1.0 - clamp(d, 0.0, 1.0);
  return t * t;
}

// How much a face's own edges darken it here, and how open its edges and
// corners leave it to the light.
struct Creases {
  edge: f32,
  open: f32,
};

// Off the face's own bits (pxl-spec-core `face`), edge `k` +u, -u, +v, -v
// and corner `k` across edges `k & 1` and `2 + (k >> 1)`. The line: along
// each crease, an edge that meets the air or one a cell rises across,
// `frame_edge_width` units wide whatever the cell's size, so cells of
// every size side by side show one line and perspective alone thins it far
// off (`line_at`). The closing in: each edge a cell rises across, and each
// corner one rises across alone, takes its share of what the last left,
// over a cell.
fn creases(h: Hit, faces: u32, p: mat2x3f) -> Creases {
  if (frame_edge_strength() <= 0.0 && frame_corner_ao() <= 0.0) {
    return Creases(0.0, 1.0);
  }
  let u = (h.axis + 1u) % 3u;
  let v = (h.axis + 2u) % 3u;
  let l = (h.at - h.corner) / h.size;
  // Each edge: how far it is, as a share of the box, the box's length
  // toward it, and the units a drawn pixel spans toward it.
  let to = vec4f(1.0 - l[u], l[u], 1.0 - l[v], l[v]);
  let pixel = max(footprint(p), vec3f(1e-9));
  let across = vec4f(pixel[u], pixel[u], pixel[v], pixel[v]);
  let extent = vec4f(h.size[u], h.size[u], h.size[v], h.size[v]);
  var edge = 0.0;
  var open = 1.0;
  for (var k = 0u; k < 4u; k++) {
    if (((faces >> (FACES_LINE + k)) & 1u) != 0u || ((faces >> (FACES_RISE + k)) & 1u) != 0u) {
      edge = max(edge, line_at(to[k] * extent[k], across[k]));
    }
    if (((faces >> (FACES_RISE + k)) & 1u) != 0u) {
      open *= 1.0 - occlusion_of(to[k]);
    }
    if (((faces >> (FACES_CORNER + k)) & 1u) != 0u) {
      let a = k & 1u;
      let b = 2u + (k >> 1u);
      open *= 1.0 - occlusion_of(length(vec2f(to[a], to[b])));
    }
  }
  let k = frame_corner_ao() * (1.0 - open);
  return Creases(edge, 1.0 - k);
}

// The face's texture coordinate: the world's pos, one repeat a unit.
fn box_uv(h: Hit) -> vec2f {
  return face_uv(vec3f(eye_phase(0u, 0), eye_phase(1u, 0), eye_phase(2u, 0)) + h.at, h.normal);
}

// This px's ray, through its middle, as the cast formed it, and what it
// met: its aim, once, for the ray and for the face's step across a px;
// the ray's words; and the face it stopped on. One reading for the lit
// pass and the depth the screen's occlusion is drawn off (screen_lit.wgsl,
// screen_depth.wgsl), so the two cannot disagree on a px.
struct Pixel {
  a: vec3f,
  size: vec2f,
  d: vec3f,
  c: vec3u,
  faces: u32,
  // A ray that met nothing leaves its first word zero: no stop is void.
  met: bool,
  h: Hit,
};

fn px_ray(clip: vec2f) -> Pixel {
  let size = vec2f(textureDimensions(rays));
  let a = aim(clip, size);
  let d = through(a);
  let at = vec2i(clip);
  let c = vec3u(textureLoad(rays, at, 0, 0).x, textureLoad(rays, at, 1, 0).x, textureLoad(rays, at, 2, 0).x);
  let faces = textureLoad(rays, at, RAY_FACES, 0).x;
  return Pixel(a, size, d, c, faces, c.x != 0u, hit_of(c.xy, faces, d));
}
