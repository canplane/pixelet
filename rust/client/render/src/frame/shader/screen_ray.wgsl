// pxl-render/frame/shader/screen_ray.wgsl

// Draws the eye's rays, one a drawn pixel (pxl-engine `ray`); screen_shade.wgsl is the
// fragment stage. Each pixel shows the face its ray entered, painted by paint.wgsl `paint` as
// the ghost's boxes are, and a pixel whose ray met nothing is left to the sky. What lies across
// each edge and corner of the face (the air, the next cell of a flat run, or a cell rising in
// front of it) is the world's own answer, asked once where the cast stopped (pxl-engine
// `face`) and handed in the pixel's own ray words. A line is drawn along an edge that meets the
// air, and the face darkens toward an edge or a corner that a cell rises across (`creases`).
// No pixel reads another pixel's ray, so every pixel of a cell is painted from the same bits.

// The rays, one layer a word: what the ray met (its value, face and zoom), how far it went (f32
// bits), the four palette indices a coarse ground cell is drawn by, and what lies around its
// face's edges and corners (`RAY_FACES`). A pixel is painted from its own words alone.
@group(0) @binding(3) var rays: texture_2d_array<u32>;
// Each value's look, indexed by the value whichever source holds it: its swatch and texture
// layer, then the color it reads as from afar, in linear light (mesh/paint/ground.rs
// `look_words`).
@group(0) @binding(20) var<storage, read> looks: array<vec4u>;
// The eye the rays were cast from (pxl-engine `eye`). It holds the eye's position in Q3 steps
// (`EYE_ORIGIN`), `FRAC_W` bits a unit, which wrap; a cell's place on the lattice is read from
// their low bits. It also holds the rays' reach, the axes and tangents each ray's direction is
// formed from, and where each pixel's ray passes through the pixel (`EYE_JITTER`).
@group(0) @binding(21) var<uniform> eye: array<vec4u, EYE_READ_VEC4S>;

fn eye_word(k: u32) -> u32 {
  return eye[k / 4u][k % 4u];
}

fn eye_vec3(k: u32) -> vec3f {
  return vec3f(bitcast<f32>(eye_word(k)), bitcast<f32>(eye_word(k + 1u)), bitcast<f32>(eye_word(k + 2u)));
}

// The unit direction along a pixel's aim (`aim`), formed from the eye's own words as the cast
// formed it (walk_node.wgsl `eye_aim`, pxl-engine cast.rs `through`), so the t a ray left is
// laid along the same direction it was cast in.
fn through(a: vec3f) -> vec3f {
  return a / sqrt(dot(a, a));
}

fn eye_tan() -> vec2f {
  return vec2f(bitcast<f32>(eye_word(EYE_TAN)), bitcast<f32>(eye_word(EYE_TAN + 1u)));
}

// Where this frame's rays pass through their pixels, as an offset from the pixels' middles in
// drawn pixels (pxl-engine `eye::JITTER`).
fn eye_jitter() -> vec2f {
  return vec2f(bitcast<f32>(eye_word(EYE_JITTER)), bitcast<f32>(eye_word(EYE_JITTER + 1u)));
}

// The direction of the ray through the fragment position `frag`, before it is made a unit. It
// is linear in `frag`, which `px_steps` relies on.
fn aim(frag: vec2f, size: vec2f) -> vec3f {
  let t = px_tan(frag + eye_jitter(), size, eye_tan());
  return eye_vec3(EYE_FORWARD) + t.x * eye_vec3(EYE_RIGHT) + t.y * eye_vec3(EYE_UP);
}

// How far behind its face a ray's depth is written (`behind`). The world steps back, so what
// is laid on its faces (the location indicator's zone, the ghosts, the markers' anchors) is
// rasterized with no offset at all. A raster on the face strays from the ray's exact depth in
// two ways. Its depth is interpolated across its corners in f32, which errs by about 2^-23 of
// the depth, and `BEHIND`, 2^-14 of the depth, stands well past that. Its corners snap to the
// subpixel grid, at the coarsest a sixteenth of a pixel (Vulkan's least
// `subPixelPrecisionBits`, 4). Half a step on each axis moves its plane at a pixel's middle by
// at most a sixteenth of its depth's slope across a pixel, taking the greater of the slopes
// across and down as a rasterizer's polygon offset does. `SNAP`, a quarter of that slope,
// stands past it fourfold.
const BEHIND: f32 = 6.103515625e-5;
const SNAP: f32 = 0.25;

struct FragOut {
  @location(0) color: vec4f,
  @builtin(frag_depth) depth: f32,
};

// The look of value `slot`. A value past the table is drawn black, with no layer.
fn look(slot: u32) -> vec4u {
  if (slot < arrayLength(&looks)) {
    return looks[slot];
  }
  return vec4u(0u, u32(NO_LAYER) << 16u, 0u, 0u);
}

fn swatch(l: vec4u) -> vec3f {
  return vec3f(unpack2x16unorm(l.x), unpack2x16unorm(l.y).x);
}

// The color a look reads as on a cell coarser than the ground's cell: its layer's mean where
// the texture pack is drawn, and its swatch where it is not (`frame_low_spec`).
fn far(l: vec4u) -> vec3f {
  return select(swatch(l), vec3f(unpack2x16unorm(l.z), unpack2x16unorm(l.w).x), frame_low_spec() < 0.5);
}

struct Appearance {
  rgb: vec3f,
  layer: f32,
};

// What the cell a pixel's ray met is drawn as, by its value, whichever source holds it. At the
// ground's cell zoom or finer it is drawn with its swatch and texture layer. A coarser cell is
// drawn in one color, as it reads from afar. Where its value is among the four palette indices
// a coarse ground cell folds, that color is the mean of the four.
fn appearance(c: vec3u) -> Appearance {
  let val = ray_val(c.x);
  let l = look(val);
  if (ray_zoom(c.x) >= GROUND_CELL_ZOOM) {
    return Appearance(swatch(l), f32(l.y >> 16u));
  }
  let fold = ray_palette_indices(c.z);
  if (any(fold == vec4u(val))) {
    // The face's own look is read once, and serves each of the four indices that name it.
    var sum = vec3f(0.0);
    for (var k = 0u; k < 4u; k++) {
      if (fold[k] == val) {
        sum += far(l);
      } else {
        sum += far(look(fold[k]));
      }
    }
    return Appearance(sum * 0.25, NO_LAYER);
  }
  return Appearance(far(l), NO_LAYER);
}

// How far the eye lies along `axis` past the start of the cell 2^k units a side that it stands
// in: the low k + FRAC_W bits of its Q3 steps, or all of them for a cell as large as the world.
fn eye_phase(axis: u32, k: i32) -> f32 {
  let bits = u32(k + i32(FRAC_W));
  let steps = eye_word(EYE_ORIGIN + axis);
  let low = select(steps, steps & ((1u << bits) - 1u), bits < 32u);
  return f32(low) * (1.0 / f32(1u << FRAC_W));
}

// Where a ray meets the face it entered, in units from the eye. The cell is the one half its
// side in from where the ray entered it, with its corner on the lattice, and `at` is held on
// that cell's face. `normal` is the face's normal and `axis` the axis it lies across. `corner`
// is the cell's least corner and `side` its side. For a cell that keeps less than its cube
// (`ray_fill`), `corner` and `side` are its box's.
struct Hit {
  at: vec3f,
  normal: vec3f,
  axis: u32,
  corner: vec3f,
  side: f32,
};

fn hit_of(c: vec2u, d: vec3f) -> Hit {
  let face = min(ray_face(c.x), 5u);
  let axis = face / 2u;
  let n = FACE_NORMAL[face];
  let k = i32(UNIT_ZOOM) - i32(ray_zoom(c.x));
  let side = exp2(f32(k));
  let e = vec3f(eye_phase(0u, k), eye_phase(1u, k), eye_phase(2u, k));
  let t = bitcast<f32>(c.y);
  let node = floor((e + d * t - n * (0.5 * fill_side(ray_fill(c.x)) * side)) / side) * side - e;
  // A cell that fills less than its cube is the box its share fills, inset about the cube's
  // center (pxl-engine cast.rs `fill_bounds`).
  let box = side * fill_side(ray_fill(c.x));
  let corner = node + vec3f(0.5 * (side - box));
  let plane = corner[axis] + select(0.0, box, n[axis] > 0.0);
  let s = select(t, plane / d[axis], d[axis] * plane > 0.0);
  return Hit(clamp(d * s, corner, corner + vec3f(box)), n, axis, corner, box);
}

// How far the hit moves along its face's plane as the pixel moves one drawn pixel across (the
// first column) and down (the second), which a rasterized face would read as dpdx and dpdy of
// its position. The steps are derived, not taken as differences, because the quad of pixels
// around an edge straddles other faces.
fn px_steps(a: vec3f, size: vec2f, h: Hit) -> mat2x3f {
  let tan = eye_tan();
  let ax = eye_vec3(EYE_RIGHT) * (2.0 * tan.x / size.x);
  let ay = eye_vec3(EYE_UP) * (-2.0 * tan.y / size.y);
  let an = dot(a, h.normal);
  let s = dot(h.at, h.normal) / an;
  return mat2x3f(s * (ax - a * (dot(ax, h.normal) / an)), s * (ay - a * (dot(ay, h.normal) / an)));
}

// The units one drawn pixel spans along each axis where the pixel's ray meets its face's
// plane.
fn footprint(p: mat2x3f) -> vec3f {
  return sqrt(p[0] * p[0] + p[1] * p[1]);
}

// The share of its depth by which a pixel's depth is written behind its face: `BEHIND`, plus
// `SNAP` times the face's own depth slope across a pixel. The depth is `near / w`, where `w` is
// the distance along the eye's forward axis `F`, so a share of the depth is the same share of
// `w`. The hit's `w` moves by `p·F` across a pixel step `p`. With the aim `a`, for which
// `a·F = 1` and the hit lies at `w·a`, and with `a_x` its change across a pixel,
// `p_x·F / w = |a_x·n| / |a·n|`.
//
// The step never lets an overlay a cell or more behind the face win over the face. The step is
// `w·|a|` times the share along the ray, while the face's plane a cell's `side` back lies
// `side·|a| / |a·n|` further along the ray, so the step reaches that plane only once
// `SNAP·w·|a_x·n| ≥ side`. But `w·|a_x|` is a pixel's width at the hit's distance, which is a
// camera grid cell there over the cosine of the ray's angle off the forward axis, and a cell
// observed there spans at least a grid cell. So the step is at most `SNAP / cos` of a cell,
// under one for any field of view narrower than 150°, however edge-on the face is seen.
// `BEHIND` adds 2^-14 of the distance, where a cell is at least 1/96 of it (pxl-engine
// `grid::CELL`).
fn behind(h: Hit, p: mat2x3f) -> f32 {
  let f = eye_vec3(EYE_FORWARD);
  return BEHIND + SNAP * max(abs(dot(p[0], f)), abs(dot(p[1], f))) / dot(h.at, f);
}

// How much a rising edge darkens a face `d` cells from it. The darkening reaches one cell and
// no further.
fn occlusion_of(d: f32) -> f32 {
  let t = 1.0 - clamp(d, 0.0, 1.0);
  return t * t;
}

// `edge` is how much the face's edge lines darken it here, and `ao` how open its edges and
// corners leave it to the light, 1 being fully open.
struct Creases {
  edge: f32,
  ao: f32,
};

// The face's creases, read from its own bits (pxl-engine `face`). Edges 0 to 3 lie at +u, -u,
// +v and -v, and corner `k` lies where edges `k & 1` and `2 + (k >> 1)` meet. A line runs along
// each crease, an edge that meets the air or one a cell rises across. The line is
// `EFFECT_EDGE_WIDTH` units wide whatever the cell's size, so cells of every size side by side
// show one line, and only perspective thins it far off (paint.wgsl `line_at`). Each edge a cell
// rises across, and each corner a cell rises across alone, darkens what the others left by its
// own share, over one cell. Both effects end at the effects' reach and fade alike before it
// (effect.wgsl `effect_share`).
fn creases(h: Hit, faces: u32, p: mat2x3f) -> Creases {
  let share = effect_share(length(h.at));
  if ((frame_edge_strength() <= 0.0 && frame_corner_ao() <= 0.0) || share <= 0.0) {
    return Creases(0.0, 1.0);
  }
  let u = (h.axis + 1u) % 3u;
  let v = (h.axis + 2u) % 3u;
  let l = (h.at - h.corner) / h.side;
  // How far each edge lies, in cells, and how many units a drawn pixel spans toward it.
  let to = vec4f(1.0 - l[u], l[u], 1.0 - l[v], l[v]);
  let px = max(footprint(p), vec3f(1e-9));
  let across = vec4f(px[u], px[u], px[v], px[v]);
  var edge = 0.0;
  var ao = 1.0;
  for (var k = 0u; k < 4u; k++) {
    if (((faces >> (FACES_LINE + k)) & 1u) != 0u || ((faces >> (FACES_RISE + k)) & 1u) != 0u) {
      edge = max(edge, line_at(to[k] * h.side, across[k]));
    }
    if (((faces >> (FACES_RISE + k)) & 1u) != 0u) {
      ao *= 1.0 - occlusion_of(to[k]);
    }
    if (((faces >> (FACES_CORNER + k)) & 1u) != 0u) {
      let a = k & 1u;
      let b = 2u + (k >> 1u);
      ao *= 1.0 - occlusion_of(length(vec2f(to[a], to[b])));
    }
  }
  let k = frame_corner_ao() * (1.0 - ao) * share;
  return Creases(edge * share, 1.0 - k);
}

// The face's texture coordinate, from the hit's position in the world, one repeat a unit.
fn box_uv(h: Hit) -> vec2f {
  return face_uv(vec3f(eye_phase(0u, 0), eye_phase(1u, 0), eye_phase(2u, 0)) + h.at, h.normal);
}

// A pixel's ray, formed as the cast formed it, and what the ray met. The aim `a` is formed
// once, for the ray's direction `d` and for the face's step across a pixel. `c` holds the
// ray's first three words and `faces` its faces word. The lit pass finds the face from the
// first two words (`hit_of`, screen_shade.wgsl).
struct Pixel {
  a: vec3f,
  size: vec2f,
  d: vec3f,
  c: vec3u,
  faces: u32,
  // A ray that met nothing leaves its first word 0, which no hit leaves, as no ray stops in
  // void.
  met: bool,
};

fn px_ray(clip: vec2f) -> Pixel {
  let size = vec2f(textureDimensions(rays));
  let a = aim(clip, size);
  let d = through(a);
  let at = vec2i(clip);
  let c = vec3u(textureLoad(rays, at, 0, 0).x, textureLoad(rays, at, 1, 0).x, textureLoad(rays, at, 2, 0).x);
  let faces = textureLoad(rays, at, RAY_FACES, 0).x;
  return Pixel(a, size, d, c, faces, c.x != 0u);
}
