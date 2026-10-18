// pxl-render/frame/shader/screen_rays.wgsl

// The eye's rays (pxl-portable `ray`), a drawn pixel each: each pixel
// the face its ray entered, painted by paint.wgsl's `paint`, as the ghost's
// boxes are. A pixel whose ray met nothing is left to the sky.

// The rays, a layer a word: what a ray met (value, face, zoom), how far it
// went (f32 bits), and the nodes its descent asked, which the steps view
// reads. A block is painted from these, its own, alone.
@group(0) @binding(3) var rays: texture_2d_array<u32>;
// Each value's swatch and layer, then the colour it reads as from afar
// (`mesh/paint/ground.rs` `look_words`), by value whichever source holds it.
@group(0) @binding(20) var<storage, read> looks: array<vec2u>;
// The eye the rays left (pxl-portable `eye`): its Q3 steps (`EYE_ORIGIN`),
// FRAC_W bits a unit and wrapping, a block's place on the lattice read off
// their low bits; whether a ray reports its steps (`EYE_STEPS`); its reach;
// and the axes and tangents each ray's direction is formed of.
@group(0) @binding(21) var<uniform> eye: array<vec4u, EYE_READ_VEC4S>;

fn eye_word(k: u32) -> u32 {
  return eye[k / 4u][k % 4u];
}

fn eye_vec3(k: u32) -> vec3f {
  return vec3f(bitcast<f32>(eye_word(k)), bitcast<f32>(eye_word(k + 1u)), bitcast<f32>(eye_word(k + 2u)));
}

// The ray through the pixel at `frag`, its middle, as the cast formed it
// (gpu-query-shader `pixel_ray`): pxl-portable cast.rs `through`'s mirror,
// off the eye's own words, so the t a ray left is laid along the very
// direction it was cast in.
fn through(frag: vec2f, size: vec2f) -> vec3f {
  let ndc = vec2f(frag.x / size.x * 2.0 - 1.0, 1.0 - frag.y / size.y * 2.0);
  let tan = vec2f(bitcast<f32>(eye_word(EYE_TAN)), bitcast<f32>(eye_word(EYE_TAN + 1u)));
  let d = eye_vec3(EYE_FORWARD) + ndc.x * tan.x * eye_vec3(EYE_RIGHT) + ndc.y * tan.y * eye_vec3(EYE_UP);
  return d / sqrt(dot(d, d));
}

// The steps a ray's colour runs up to, on a log scale, the same on every
// screen so two compare.
const STEPS_MOST: f32 = 1024.0;

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
  layer: f32,
};

// What the block a pixel's ray met is drawn as, by its value alone,
// whichever source holds it: at the ground's cell or finer, its swatch and
// its layer; coarser, its fold's majority as it reads from
// afar (doc/minutes/20261005-the-ground-is-a-carto-palette.md).
fn drawn(w: u32) -> Drawn {
  let l = look(ray_value(w));
  if (ray_zoom(w) >= GROUND_CELL) {
    return Drawn(linear_of(l.x), f32(l.x >> 24u));
  }
  return Drawn(far(l), f32(NO_LAYER));
}

// Along `axis`, how far the eye is past the start of the block of side
// 2^k units it stands in: its steps' low k + FRAC_W bits, every bit at the
// world's side.
fn eye_phase(axis: u32, k: i32) -> f32 {
  let bits = u32(k + i32(FRAC_W));
  let steps = eye_word(EYE_ORIGIN + axis);
  let low = select(steps, steps & ((1u << bits) - 1u), bits < 32u);
  return f32(low) / f32(1u << FRAC_W);
}

// Where a ray meets the face it entered, from the eye, kept on the block
// (the block half its side in from where the ray entered it, its corner on
// the lattice, and the face's plane through it), and the face's normal.
struct Hit {
  at: vec3f,
  normal: vec3f,
};

fn hit_of(c: vec2u, d: vec3f) -> Hit {
  let face = min(ray_face(c.x), 5u);
  let axis = face / 2u;
  let n = FACE_NORMAL[face];
  let k = i32(UNIT_ZOOM) - i32(ray_zoom(c.x));
  let side = exp2(f32(k));
  let e = vec3f(eye_phase(0u, k), eye_phase(1u, k), eye_phase(2u, k));
  let t = bitcast<f32>(c.y);
  let corner = floor((e + d * t - n * (0.5 * side)) / side) * side - e;
  let plane = corner[axis] + select(0.0, side, n[axis] > 0.0);
  let s = select(t, plane / d[axis], d[axis] * plane > 0.0);
  return Hit(clamp(d * s, corner, corner + vec3f(side)), n);
}

// The face's texture coordinate: the world's position, one repeat a unit.
fn box_uv(h: Hit) -> vec2f {
  let p = vec3f(eye_phase(0u, 0), eye_phase(1u, 0), eye_phase(2u, 0)) + h.at;
  if (abs(h.normal.z) > 0.5) {
    return p.xy;
  }
  return select(vec2f(p.x, -p.z), vec2f(p.y, -p.z), abs(h.normal.x) > 0.5);
}

@fragment
fn fs(v: VertexOut) -> FragOut {
  // This pixel's ray, through its middle, as the cast formed it.
  let d = through(v.clip.xy, vec2f(textureDimensions(rays)));
  let at = vec2i(v.clip.xy);
  let c = vec2u(textureLoad(rays, at, 0, 0).x, textureLoad(rays, at, 1, 0).x);
  // A ray that met nothing leaves its first word zero: no stop is void.
  let met = c.x != 0u;
  let h = hit_of(c, d);
  // Taken before any branch: a derivative needs the quad whole.
  let uv = box_uv(h);
  let ddx = dpdx(uv);
  let ddy = dpdy(uv);
  if (eye_word(EYE_STEPS) != 0u) {
    // A ray that met nothing at the end of what the eye observes, nearer than
    // the far plane's 0 the sky stands at.
    let t = select(bitcast<f32>(eye_word(EYE_REACH)), bitcast<f32>(c.y), met);
    let clip = frame_view_proj() * vec4f(d * t, 1.0);
    var o: FragOut;
    // The nodes asked, written and read only in this view.
    let steps = textureLoad(rays, at, 2, 0).x;
    o.color = vec4f(turbo(log2(1.0 + f32(steps)) / log2(1.0 + STEPS_MOST)), 1.0);
    o.depth = clip.z / clip.w;
    return o;
  }
  if (!met) {
    discard;
  }
  let painted = drawn(c.x);
  let s = Surface(
    painted.rgb,
    painted.layer,
    h.normal,
    0.0,
    h.at,
    uv,
    ddx,
    ddy,
    screen_open(v.clip.xy),
    seen_from_eye(h.at),
  );
  let clip = frame_view_proj() * vec4f(h.at, 1.0);
  var o: FragOut;
  o.color = vec4f(finish(paint(s), v.clip.xy), 1.0);
  o.depth = clip.z / clip.w;
  return o;
}
