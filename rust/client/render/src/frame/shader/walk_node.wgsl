// pxl-render/walk_node.wgsl

// What a walk works out of a node, every walk's alike, over the eye's words (eye_bind.wgsl):
// a ray to walk, where it starts and ends, a node's planes and its index, its bucket, the zoom
// asked at a distance, a ray's span in a box (pxl-engine cast.rs).

/// A cut as the eye holds it: its index, then its zoom.
fn eye_cut(k: u32) -> vec4<i32> {
  return vec4<i32>(i32(eye_word(k)), i32(eye_word(k + 1u)), i32(eye_word(k + 2u)), i32(eye_word(k + 3u)));
}

/// A ray to walk: from `origin`, at the zooms `range` holds it to; `inv` its direction's
/// reciprocal on each axis, 0 where it does not move along one (`Ray::inverse`), the eye's
/// worked out once by the GPU (`eye_aim`), the light's the host's (light_aim.wgsl
/// `light_aim`). What its origin decides beside (walk.wgsl `walk`): its stop rule, `until`
/// (`cast::Until`); a beam ray's line, `line`, `NONE` for any other ray; and how far apart its
/// beam's rays lie, `spacing`, `across` then `angle` (`cast::Spacing`), none for any other.
struct Aim {
  origin: vec3<i32>,
  dir: vec3<f32>,
  inv: vec3<f32>,
  t_max: f32,
  range: vec2<u32>,
  until: u32,
  line: u32,
  spacing: vec2<f32>,
}

/// A ray's stop rule (`cast::Until`): at what it cannot see through, but not in the cell it
/// starts in, and where it enters the box a cell keeping less than its cube is; or a beam
/// ray's, at what is not transparent and at a split too narrow to share.
const UNTIL_OPAQUE = 0u;
const UNTIL_BEAM = 2u;

/// The eye's ray at the tangents `tan_at`: the one place a GPU works out the eye's way and its
/// reciprocal, a division and a square root its own (`cast::through`, `Ray::new`), so every
/// pass that walks or marks an eye's ray takes it here.
fn eye_aim(tan_at: vec2<f32>) -> Aim {
  let aimed = eye_v3(FORWARD) + tan_at.x * eye_v3(RIGHT) + tan_at.y * eye_v3(UP);
  let dir = aimed / length(aimed);
  return Aim(
    vec3<i32>(i32(eye_word(ORIGIN)), i32(eye_word(ORIGIN + 1u)), i32(eye_word(ORIGIN + 2u))),
    dir,
    ray_inv(ray_dir(dir)),
    eye_f32(FAR),
    vec2<u32>(eye_word(RANGE), eye_word(RANGE + 1u)),
    UNTIL_OPAQUE,
    NONE,
    vec2<f32>(0.0),
  );
}

/// The beam ray `aim` of a beam whose rays lie `spacing` apart, its line at `line`.
fn beam_ray(aim: Aim, line: u32, spacing: vec2<f32>) -> Aim {
  return Aim(aim.origin, aim.dir, aim.inv, aim.t_max, aim.range, UNTIL_BEAM, line, spacing);
}

/// The ray's way, its axes it hardly moves along none (`Ray::new`).
fn ray_dir(aimed: vec3<f32>) -> vec3<f32> {
  return select(aimed, vec3<f32>(0.0), abs(aimed) < vec3<f32>(1.17549435e-38));
}

fn ray_inv(dir: vec3<f32>) -> vec3<f32> {
  return select(vec3<f32>(1.0) / dir, vec3<f32>(0.0), dir == vec3<f32>(0.0));
}

/// A ray's state between steps, where a walk starts (resume.wgsl): the node holding where the
/// ray is at `t`; what was handed that node's parent, `parent`, unless the node is asked from
/// the top, `top`; and the steps the walk took before (`cast::walk_at_most`).
struct Start {
  t: f32,
  corner: vec3<u32>,
  zoom: u32,
  roots: vec3<i32>,
  parent: vec4<u32>,
  top: bool,
  step: u32,
}

/// The node holding where the ray is at `t`, asked from the top, no step taken.
fn start_at(t: f32, corner: vec3<u32>, zoom: u32, roots: vec3<i32>) -> Start {
  return Start(t, corner, zoom, roots, vec4<u32>(0u), true, 0u);
}

/// From the root of the origin, at 0.
fn root_start() -> Start {
  return start_at(0.0, vec3<u32>(0u), 0u, vec3<i32>(0));
}

/// Where a walk ended or stopped, and its state there. `ends`: 1 at a cell it stops at, 2 where
/// one lacks, 3 gone to its reach, 4 left to the walk that asks, 5 out of the steps it was
/// given, the last two at `t`, at the node `zoom`, `corner` and `roots` name, which it goes on
/// from with `parent`, `top` and `step` (`Start`); `visits` the nodes it entered.
struct Walked {
  ends: u32,
  zoom: u32,
  corner: vec3<u32>,
  roots: vec3<i32>,
  asset: vec2<u32>,
  axis: u32,
  t: f32,
  visits: u32,
  parent: vec4<u32>,
  top: bool,
  step: u32,
}

/// Units from the origin at `og` to `p`, in the root `roots` from it (pxl-engine
/// cast.rs `units`).
fn units(p: vec3<u32>, og: vec3<u32>, roots: vec3<i32>) -> vec3<f32> {
  let whole = vec3<i32>(p >> vec3<u32>(FRAC_W)) - vec3<i32>(og >> vec3<u32>(FRAC_W))
    + (roots << vec3<u32>(UNIT_ZOOM));
  let frac = vec3<i32>(p & vec3<u32>(255u)) - vec3<i32>(og & vec3<u32>(255u));
  return vec3<f32>(whole) + vec3<f32>(frac) * (1.0 / f32(1u << FRAC_W));
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

/// The node `c` of `zoom`, hashed.
fn node_hash(zoom: u32, c: vec3<i32>) -> u32 {
  var h = zoom * 0x85EBCA77u;
  h = mix_hash(h, u32(c.x));
  h = mix_hash(h, u32(c.y));
  h = mix_hash(h, u32(c.z));
  return h ^ (h >> 16u);
}

/// The first slot of its bucket.
fn bucket_of(zoom: u32, c: vec3<i32>) -> u32 {
  return node_hash(zoom, c) & (ENTRIES - 1u) & ~(BUCKET - 1u);
}

/// Only nodes under the world's roots are kept.
fn keeps(zoom: u32, c: vec3<i32>) -> bool {
  if (zoom >= 32u) {
    return false;
  }
  let v = c >> vec3<u32>(zoom);
  return (v.x == 0i || v.x == -1i) && (v.y == 0i || v.y == -1i) && (v.z == 0i || v.z == -1i);
}

/// A vector's length squared, summed in pxl-engine's order (cast.rs `squared`). A device may
/// fuse a product into the sum, which puts it a last place off the Rust's (tests/walk.rs
/// `hand_zoom_on_a_split_distance`).
fn squared(v: vec3<f32>) -> f32 {
  return v.x * v.x + v.y * v.y + v.z * v.z;
}

/// The zoom asked at `r2`, a distance squared (`Detail::at`, `grid::cell_zoom_at`).
fn zoom_at(r2: f32, range: vec2<u32>) -> u32 {
  return zoom_in_splits(r2 * INV_SPLIT2, range);
}

/// The zoom asked at `x2`, a distance squared in split distances squared
/// (`grid::cell_zoom_in_splits`).
fn zoom_in_splits(x2: f32, range: vec2<u32>) -> u32 {
  let e = (i32((bitcast<u32>(x2) >> 23u) & 255u) - 127i) >> 1u;
  let z = u32(clamp(i32(UNIT_ZOOM) - e, 0i, i32(FINEST_ZOOM)));
  return min(max(z, range.x), range.y);
}

/// How far apart a beam's rays lie at `d2` from the walk's origin, a distance squared, its
/// spacing `across` then `angle` (`Spacing::width`). Only a GPU walks a beam, so it alone
/// decides this, every pass through this function; its square root may lie off the Rust's
/// (tests/walk.rs `WIDTH_ULPS`).
fn spacing_width(spacing: vec2<f32>, d2: f32) -> f32 {
  return spacing.x + spacing.y * sqrt(d2);
}

/// Whether a split `side` wide is too narrow for a beam ray to share where its beam's rays lie
/// `width` apart (`cast::narrow`). The bound is pxl-engine's `NARROW_SQUARED`, 8, not
/// `NARROW` squared in f32, which rounds a place under it.
fn narrow(side: f32, width: f32) -> bool {
  return side * side < NARROW_SQUARED * (width * width);
}

/// Where a ray enters a plane it reaches at `t`, no nearer than the origin, +0 there as
/// pxl-engine's `greater(t, 0.0)` gives it where `max` may give −0 (`entry_plane`).
fn past_origin(t: f32) -> f32 {
  return select(0.0, t, t > 0.0);
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
