// pxl-render/frame/shader/screen_cells.wgsl

// The rays' cells (`frame/screen_cells.rs`), each drawn straight into its own
// area of the screen: the lane's cells across and up, at equal steps of angle
// over the view, the block's swatch lit by the face its ray entered through,
// at the depth its ray stopped at. A cell whose ray met nothing is left to
// the sky.

// The lane (`frame/screen_cells.rs`): the cells across and up and whether a
// cell is coloured by its ray's steps, then a cell each, what its ray met
// (value, kind, face), how far it went (f32 bits) and the nodes its descent
// asked.
@group(0) @binding(2) var<storage, read> cells: array<vec4u>;
// Each ground look's swatch, then each placed value's.
@group(0) @binding(3) var<storage, read> palette: array<u32>;

const KIND_GROUND: u32 = 1u;
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

@fragment
fn fs(v: VertexOut) -> FragOut {
  let m = sky_ray();
  let dir = m * vec3f(v.ndc, 1.0);
  let forward = length(m[2]);
  let z = dot(dir, m[2] / forward);
  // Each axis's angle from the view's edge, counted in cells.
  let angle = vec2f(atan2(dot(dir, normalize(m[0])), z), atan2(dot(dir, normalize(m[1])), z));
  let half = vec2f(atan2(length(m[0]), forward), atan2(length(m[1]), forward));
  let across = cells[0].xy;
  let at = vec2i(floor((angle + half) / (2.0 * half) * vec2f(across)));
  if (any(at < vec2i(0)) || any(vec2u(at) >= across)) {
    discard;
  }
  let c = cells[1u + u32(at.y) * across.x + u32(at.x)];
  let kind = (c.x >> 16u) & 3u;
  if (cells[0].z != 0u) {
    let t = select(bitcast<f32>(c.y), FAR, kind == 0u);
    let clip = frame_view_proj() * vec4f(normalize(dir) * t, 1.0);
    var o: FragOut;
    o.color = vec4f(turbo(log2(1.0 + f32(c.z)) / log2(1.0 + STEPS_MOST)), 1.0);
    o.depth = clip.z / clip.w;
    return o;
  }
  if (kind == 0u) {
    discard;
  }
  let value = c.x & 0xffffu;
  let slot = select(GROUND_LOOKS + value, value, kind == KIND_GROUND);
  let rgb = select(0u, palette[slot], slot < arrayLength(&palette));
  let face = min((c.x >> 18u) & 7u, 5u);
  let p = normalize(dir) * bitcast<f32>(c.y);
  let clip = frame_view_proj() * vec4f(p, 1.0);
  let swatch = srgb_to_linear(unpack4x8unorm(rgb).rgb);
  let lit = 0.45 + 0.55 * max(dot(FACE_NORMAL[face], frame_sun_dir()), 0.0);
  var o: FragOut;
  o.color = vec4f(to_srgb(swatch * lit), 1.0);
  o.depth = clip.z / clip.w;
  return o;
}
