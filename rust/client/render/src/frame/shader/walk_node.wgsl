// pxl-render/walk_node.wgsl

// What a walk works out of a node, every walk's alike: the eye's words, a ray to walk, where
// it starts and ends, a node's planes and its index, its bucket, the zoom asked at a distance,
// a ray's span in a box (pxl-engine cast.rs).

@group(0) @binding(0) var<uniform> eye: array<vec4<u32>, EYE_VEC4S>;

fn eye_word(k: u32) -> u32 {
  return eye[k / 4u][k % 4u];
}

fn eye_f32(k: u32) -> f32 {
  return bitcast<f32>(eye_word(k));
}

fn eye_v3(k: u32) -> vec3<f32> {
  return vec3<f32>(eye_f32(k), eye_f32(k + 1u), eye_f32(k + 2u));
}

/// A cut as the eye holds it: its index, then its zoom.
fn eye_cut(k: u32) -> vec4<i32> {
  return vec4<i32>(i32(eye_word(k)), i32(eye_word(k + 1u)), i32(eye_word(k + 2u)), i32(eye_word(k + 3u)));
}

/// A ray to walk: from `origin`, at the zooms `range` holds it to; `seed`, other rays'
/// and other frames', says which asks again (walk.wgsl `revisits`).
struct Aim {
  origin: vec3<i32>,
  dir: vec3<f32>,
  t_max: f32,
  range: vec2<u32>,
  seed: u32,
}

/// The eye's ray at the tangents `tan_at`.
fn eye_aim(tan_at: vec2<f32>) -> Aim {
  let aimed = eye_v3(FORWARD) + tan_at.x * eye_v3(RIGHT) + tan_at.y * eye_v3(UP);
  return Aim(
    vec3<i32>(i32(eye_word(ORIGIN)), i32(eye_word(ORIGIN + 1u)), i32(eye_word(ORIGIN + 2u))),
    aimed / length(aimed),
    eye_f32(FAR),
    vec2<u32>(eye_word(RANGE), eye_word(RANGE + 1u)),
    mix_hash(mix_hash(eye_word(JITTER), bitcast<u32>(tan_at.x)), bitcast<u32>(tan_at.y)),
  );
}

/// The ray's way, its axes it hardly moves along none (`Ray::new`).
fn ray_dir(aimed: vec3<f32>) -> vec3<f32> {
  return select(aimed, vec3<f32>(0.0), abs(aimed) < vec3<f32>(1.17549435e-38));
}

fn ray_inv(dir: vec3<f32>) -> vec3<f32> {
  return select(vec3<f32>(1.0) / dir, vec3<f32>(0.0), dir == vec3<f32>(0.0));
}

/// Where a walk starts: the node holding where the ray is at `t`, and the nodes its beam ray
/// visited (`visits`, resume.wgsl).
struct Start {
  t: f32,
  corner: vec3<u32>,
  zoom: u32,
  roots: vec3<i32>,
  visits: u32,
}

/// From the root of the origin, at 0.
fn root_start() -> Start {
  return Start(0.0, vec3<u32>(0u), 0u, vec3<i32>(0), 0u);
}

/// Where a walk ended. `ends`: 1 at a cell it stops at, 2 where one lacks, 3 gone to
/// its reach, anything else left.
struct Walked {
  ends: u32,
  zoom: u32,
  corner: vec3<u32>,
  roots: vec3<i32>,
  asset: vec2<u32>,
  axis: u32,
  t: f32,
  visits: u32,
}

/// Units from the origin at `og` to `p`, in the root `roots` from it (pxl-engine
/// cast.rs `units`).
fn units(p: vec3<u32>, og: vec3<u32>, roots: vec3<i32>) -> vec3<f32> {
  let whole = vec3<i32>(p >> vec3<u32>(FRAC_W)) - vec3<i32>(og >> vec3<u32>(FRAC_W))
    + (roots << vec3<u32>(UNIT_ZOOM));
  let frac = vec3<i32>(p & vec3<u32>(255u)) - vec3<i32>(og & vec3<u32>(255u));
  return vec3<f32>(whole) + vec3<f32>(frac) / f32(1u << FRAC_W);
}

/// A node's side at `zoom` less one (`under`).
fn under(zoom: u32) -> u32 {
  return select(0xffffffffu >> zoom, 0u, zoom >= FINEST_ZOOM);
}

/// Its far planes: the next node's corner, in the next root past its own.
fn far_of(corner: vec3<u32>, u: u32, og: vec3<u32>, roots: vec3<i32>) -> vec3<f32> {
  let last = corner + vec3<u32>(u);
  let over = select(vec3<i32>(0), vec3<i32>(1), last == vec3<u32>(0xffffffffu));
  return units(last + vec3<u32>(1u), og, roots + over);
}

/// The world's node on each axis, X named as the world names it (`Node::idx`).
fn idx_of(corner: vec3<u32>, zoom: u32, roots: vec3<i32>, r0: vec3<i32>) -> vec3<i32> {
  let root = r0 + roots;
  let high = select(vec3<u32>(root << vec3<u32>(zoom & 31u)), vec3<u32>(0u), zoom >= FINEST_ZOOM);
  let low = select(corner >> vec3<u32>((FINEST_ZOOM - zoom) & 31u), vec3<u32>(0u), zoom == 0u);
  let n = vec3<i32>(high + low);
  let x = select((n.x << ((32u - zoom) & 31u)) >> ((32u - zoom) & 31u), 0i, zoom == 0u);
  // Y's from the world's south edge and Z's from its floor (`overwrite::y_start`,
  // `overwrite::z_start`).
  let y_start = select(1i << ((zoom - 1u) & 31u), 0i, zoom == 0u);
  let z_start = select(1i << ((zoom - SHARD_ZOOM - 1u) & 31u), 0i, zoom <= SHARD_ZOOM);
  return vec3<i32>(x, n.y - y_start, n.z - z_start);
}

fn mix_hash(h: u32, v: u32) -> u32 {
  let m = (h ^ v) * 0x9E3779B1u;
  return (m << 15u) | (m >> 17u);
}

/// `pyramid::hash`.
fn node_hash(zoom: u32, c: vec3<i32>) -> u32 {
  var h = zoom * 0x85EBCA77u;
  h = mix_hash(h, u32(c.x));
  h = mix_hash(h, u32(c.y));
  h = mix_hash(h, u32(c.z));
  return h ^ (h >> 16u);
}

/// The first slot of its bucket (`pyramid::entry_of`).
fn bucket_of(zoom: u32, c: vec3<i32>) -> u32 {
  return node_hash(zoom, c) & (ENTRIES - 1u) & ~(BUCKET - 1u);
}

/// Only nodes under the world's roots are kept (`pyramid::keeps`).
fn keeps(zoom: u32, c: vec3<i32>) -> bool {
  if (zoom >= 32u) {
    return false;
  }
  let v = c >> vec3<u32>(zoom);
  return (v.x == 0i || v.x == -1i) && (v.y == 0i || v.y == -1i) && (v.z == 0i || v.z == -1i);
}

/// The zoom asked at `r2`, a distance squared (`Detail::at`, `grid::cell_zoom_at`).
fn zoom_at(r2: f32, range: vec2<u32>) -> u32 {
  let x2 = r2 / (ZOOM_SPLIT * ZOOM_SPLIT);
  let e = (i32((bitcast<u32>(x2) >> 23u) & 255u) - 127i) >> 1u;
  let z = u32(clamp(i32(UNIT_ZOOM) - e, 0i, i32(FINEST_ZOOM)));
  return min(max(z, range.x), range.y);
}

/// Near and far on each axis (`planes`), branchless: an axis it runs beside gives
/// the span whole inside the slab and none outside.
struct Span {
  near: vec3<f32>,
  far: vec3<f32>,
}

fn span_of(d: vec3<f32>, inv: vec3<f32>, lo: vec3<f32>, hi: vec3<f32>, t: f32, t_max: f32) -> Span {
  let a = lo * inv;
  let b = hi * inv;
  let up = d > vec3<f32>(0.0);
  let moves = d != vec3<f32>(0.0);
  let inside = (lo <= vec3<f32>(0.0)) & (vec3<f32>(0.0) < hi);
  let beside_near = select(vec3<f32>(t_max), vec3<f32>(t), inside);
  let beside_far = select(vec3<f32>(t), vec3<f32>(t_max), inside);
  return Span(
    select(beside_near, select(b, a, up), moves),
    select(beside_far, select(a, b, up), moves),
  );
}

fn largest(v: vec3<f32>) -> f32 {
  return max(max(v.x, v.y), v.z);
}

fn smallest(v: vec3<f32>) -> f32 {
  return min(min(v.x, v.y), v.z);
}

/// The axis crossed: of those it moves along, the last plane reached (`last`) or
/// the first, the lowest at a tie, the third where it moves along none (`crossing`).
fn crossing(d: vec3<f32>, t: vec3<f32>, last: bool) -> u32 {
  var e = 2u;
  if (d.x != 0.0) {
    e = 0u;
  } else if (d.y != 0.0) {
    e = 1u;
  }
  if (e < 1u && d.y != 0.0 && ((last && t.y > t[e]) || (!last && t.y < t[e]))) {
    e = 1u;
  }
  if (e < 2u && d.z != 0.0 && ((last && t.z > t[e]) || (!last && t.z < t[e]))) {
    e = 2u;
  }
  return e;
}

/// Step `e` of a box on the grid's own planes, rounded out by `out` where finer
/// than the finest grid's count (`meets_box`'s `step`).
fn box_step(e: vec3<u32>, u: u32, out: u32) -> vec3<u32> {
  if (u >= BOX_STEPS - 1u) {
    return e * ((u >> BOX_BITS) + 1u);
  }
  return (e * (u + 1u) + vec3<u32>(out)) >> vec3<u32>(BOX_BITS);
}
