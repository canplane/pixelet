// pxl-render/walk_node.wgsl

// What every walk works out about a ray and a node, the same for every walk, over the eye's
// words (eye_bind.wgsl): the ray to walk, where a walk starts and ends, a node's planes, its
// index and its bucket in the node cache, the zoom asked at a distance, and a ray's span in a
// box. Each function computes what its pxl-engine twin in cast.rs or node_cache.rs computes.

/// The cut at word `k` of the eye's words, as the eye holds it: its index, then its zoom.
fn eye_cut(k: u32) -> vec4<i32> {
  return vec4<i32>(i32(eye_word(k)), i32(eye_word(k + 1u)), i32(eye_word(k + 2u)), i32(eye_word(k + 3u)));
}

/// A ray to walk, from `origin` along `dir` as far as `t_max`, asking zooms within `range`.
/// `inv` is the direction's reciprocal on each axis, 0 where the ray does not move along it
/// (pxl-engine `Ray::inverse`). The GPU works out the eye's rays once (`eye_aim`), and the host
/// works out the light's (light_aim.wgsl `light_aim`). The pass that makes the ray decides the
/// rest (walk.wgsl `walk`). `until` is its stop rule (pxl-engine `cast::Until`). `line` is a
/// beam ray's line, `NONE` for any other ray. `spacing` is how far apart a beam's rays lie,
/// `across` then `angle` (pxl-engine `cast::Spacing`), and 0 for any other ray.
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

/// A ray's stop rule (pxl-engine `cast::Until`). `UNTIL_OPAQUE` stops at the first cell the ray
/// cannot see through, though not in the cell it starts in, and at a cell that keeps less than
/// its cube only where the ray enters that cell's box. `UNTIL_BEAM`, a beam ray's rule, stops at
/// the first cell that is not transparent and at the first split too narrow for the beam's rays
/// to share.
const UNTIL_OPAQUE = 0u;
const UNTIL_BEAM = 2u;

/// The eye's ray at the tangents `tan_at` (pxl-engine `cast::through`, `Ray::new`). This is the
/// one place the GPU works out an eye's ray's direction and its reciprocal. A GPU's division
/// and square root round in its own way, so every pass that walks or marks an eye's ray takes
/// it from here, and all of them walk the same ray.
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

/// The ray `aim` as a beam ray, whose line lies at `line` and whose beam's rays lie `spacing`
/// apart.
fn beam_ray(aim: Aim, line: u32, spacing: vec2<f32>) -> Aim {
  return Aim(aim.origin, aim.dir, aim.inv, aim.t_max, aim.range, UNTIL_BEAM, line, spacing);
}

/// The ray's direction, with a component smaller than f32's least normal magnitude taken as 0
/// (pxl-engine `Ray::new`).
fn ray_dir(aimed: vec3<f32>) -> vec3<f32> {
  return select(aimed, vec3<f32>(0.0), abs(aimed) < vec3<f32>(1.17549435e-38));
}

fn ray_inv(dir: vec3<f32>) -> vec3<f32> {
  return select(vec3<f32>(1.0) / dir, vec3<f32>(0.0), dir == vec3<f32>(0.0));
}

/// A ray's state between steps, where a walk starts (resume.wgsl). `corner`, `zoom` and `roots`
/// name the node that holds the ray's point at `t`. `parent` holds what was handed the node's
/// parent, unless `top` says the node is asked from the top. `step` counts the steps the walk
/// took before (pxl-engine `cast::walk_at_most`).
struct Start {
  t: f32,
  corner: vec3<u32>,
  zoom: u32,
  roots: vec3<i32>,
  parent: vec3<u32>,
  top: bool,
  step: u32,
}

/// The start at the node that holds the ray's point at `t`, asked from the top, with no step
/// taken.
fn start_at(t: f32, corner: vec3<u32>, zoom: u32, roots: vec3<i32>) -> Start {
  return Start(t, corner, zoom, roots, vec3<u32>(0u), true, 0u);
}

fn root_start() -> Start {
  return start_at(0.0, vec3<u32>(0u), 0u, vec3<i32>(0));
}

/// Where a walk ended or stopped, and its state there. `ends` is 1 at a cell it stops at, 2 at
/// a node that lacks, 3 where it went on to its reach, 4 where it is left to the walk that asks
/// the world, and 5 where it ran out of the steps it was given. In the last two cases it stopped
/// at `t`, at the node `zoom`, `corner` and `roots` name, and goes on from there with `parent`,
/// `top` and `step` (`Start`). `visits` counts the nodes it entered.
struct WalkEnd {
  ends: u32,
  zoom: u32,
  corner: vec3<u32>,
  roots: vec3<i32>,
  asset: vec2<u32>,
  axis: u32,
  t: f32,
  visits: u32,
  parent: vec3<u32>,
  top: bool,
  step: u32,
}

/// The units from the origin at `og` to the point `p`, which lies in the root `roots` counted
/// from the origin's (pxl-engine cast.rs `units`).
fn units(p: vec3<u32>, og: vec3<u32>, roots: vec3<i32>) -> vec3<f32> {
  let whole = vec3<i32>(p >> vec3<u32>(FRAC_W)) - vec3<i32>(og >> vec3<u32>(FRAC_W))
    + (roots << vec3<u32>(UNIT_ZOOM));
  let frac = vec3<i32>(p & vec3<u32>(255u)) - vec3<i32>(og & vec3<u32>(255u));
  return vec3<f32>(whole) + vec3<f32>(frac) * (1.0 / f32(1u << FRAC_W));
}

/// A node's side at `zoom` in steps of the finest grid, less one (pxl-engine `under`).
fn under(zoom: u32) -> u32 {
  return select(0xffffffffu >> zoom, 0u, zoom >= FINEST_ZOOM);
}

/// The far planes of the node at `corner`, whose side less one is `u`: the next node's corner,
/// which lies in the next root where the node ends at its own root's edge.
fn far_of(corner: vec3<u32>, u: u32, og: vec3<u32>, roots: vec3<i32>) -> vec3<f32> {
  let last = corner + vec3<u32>(u);
  let over = select(vec3<i32>(0), vec3<i32>(1), last == vec3<u32>(0xffffffffu));
  return units(last + vec3<u32>(1u), og, roots + over);
}

/// The world's index of the node at `corner` and `zoom` on each axis, with X wrapped as the
/// world names it (pxl-engine `cast::Node::idx`). `roots` counts from the root `r0`.
fn idx_of(corner: vec3<u32>, zoom: u32, roots: vec3<i32>, r0: vec3<i32>) -> vec3<i32> {
  let root = r0 + roots;
  let high = select(vec3<u32>(root << vec3<u32>(zoom & 31u)), vec3<u32>(0u), zoom >= FINEST_ZOOM);
  let low = select(corner >> vec3<u32>((FINEST_ZOOM - zoom) & 31u), vec3<u32>(0u), zoom == 0u);
  let n = vec3<i32>(high + low);
  let x = select((n.x << ((32u - zoom) & 31u)) >> ((32u - zoom) & 31u), 0i, zoom == 0u);
  // Y counts from the world's south edge and Z from its floor (pxl-spec `zoom::y_start`,
  // `zoom::z_start`).
  let y_start = select(1i << ((zoom - 1u) & 31u), 0i, zoom == 0u);
  let z_start = select(1i << ((zoom - SHARD_ZOOM - 1u) & 31u), 0i, zoom <= SHARD_ZOOM);
  return vec3<i32>(x, n.y - y_start, n.z - z_start);
}

fn mix_hash(h: u32, v: u32) -> u32 {
  let m = (h ^ v) * 0x9E3779B1u;
  return (m << 15u) | (m >> 17u);
}

/// The hash of the node `c` at `zoom`, the port of pxl-engine `node_cache::node_hash`
/// (tests/walk.rs `hand_keeps_as_the_engine`).
fn node_hash(zoom: u32, c: vec3<i32>) -> u32 {
  var h = zoom * 0x85EBCA77u;
  h = mix_hash(h, u32(c.x));
  h = mix_hash(h, u32(c.y));
  h = mix_hash(h, u32(c.z));
  return h ^ (h >> 16u);
}

/// The first slot of the bucket that may keep the node `c` at `zoom`.
fn bucket_of(zoom: u32, c: vec3<i32>) -> u32 {
  return node_hash(zoom, c) & (ENTRIES - 1u) & ~(BUCKET - 1u);
}

/// Whether the held key `key` keeps its node, at `zoom`, for a walk asking it at `zooms`, the
/// far zoom and then the near one `ZOOM_BITS` above it (pxl-engine `node_cache::holds_for`). It
/// does where it keeps the node at those zooms. It also does where it keeps the node as a cell
/// by its node alone (`CELL_FAR`) and the query answers that cell at these zooms too, because
/// they ask at its zoom or coarser, or because the cell is whole.
fn holds_for(key: u32, zoom: u32, zooms: u32) -> bool {
  let kz = key >> TAG_BITS;
  if (kz == zooms) {
    return true;
  }
  return (kz & ((1u << ZOOM_BITS) - 1u)) == CELL_FAR && ((zooms >> ZOOM_BITS) <= zoom || (kz >> ZOOM_BITS) != 0u);
}

/// Whether a slot may keep the node `c` at `zoom`: its zoom is coarser than the finest, and its
/// index on each axis lies within twice the world's span around the world (pxl-engine
/// `node_cache::keeps`; tests/walk.rs `hand_keeps_as_the_engine`).
fn keeps(zoom: u32, c: vec3<i32>) -> bool {
  if (zoom >= 32u) {
    return false;
  }
  let v = c >> vec3<u32>(zoom);
  return (v.x == 0i || v.x == -1i) && (v.y == 0i || v.y == -1i) && (v.z == 0i || v.z == -1i);
}

/// A vector's length squared, summed in pxl-engine's order (cast.rs `squared`). A device may
/// fuse a product into the sum, which can put the result one last place off the Rust's
/// (tests/walk.rs `hand_zoom_on_a_split_distance`).
fn squared(v: vec3<f32>) -> f32 {
  return v.x * v.x + v.y * v.y + v.z * v.z;
}

/// The zooms at which a node whose planes lie at `lo` and `hi` from the origin is asked: the
/// zoom at its farthest corner, and `ZOOM_BITS` above it the zoom at its nearest point
/// (pxl-engine `cast::ask_zooms`, `node_dist_sq`). A range of one zoom, as the light's is, gives
/// that zoom for both.
fn asked_zooms(lo: vec3<f32>, hi: vec3<f32>, range: vec2<u32>) -> u32 {
  if (range.x == range.y) {
    return range.x | (range.x << ZOOM_BITS);
  }
  return zoom_at(squared(max(-lo, hi)), range) | (zoom_at(squared(min(max(lo, vec3<f32>(0.0)), hi)), range) << ZOOM_BITS);
}

/// The root that holds `origin` on each axis (pxl-engine `cast::roots_of`), from which
/// `idx_of` counts a node's roots.
fn root_of(origin: vec3<i32>) -> vec3<i32> {
  return vec3<i32>(origin.x >> 31u, 0i, i32(u32(origin.z) + Z_START) >> 31u);
}

/// The zoom asked at the squared distance `r2`, held within `range` (pxl-engine `Detail::at`,
/// `grid::cell_zoom_at`).
fn zoom_at(r2: f32, range: vec2<u32>) -> u32 {
  return zoom_in_splits(r2 * INV_SPLIT2, range);
}

/// The zoom asked at `x2`, a squared distance counted in squared split distances, held within
/// `range` (pxl-spec `grid::cell_zoom_in_splits`). The zoom falls by one each time the
/// distance doubles, read from the float's exponent.
fn zoom_in_splits(x2: f32, range: vec2<u32>) -> u32 {
  let e = (i32((bitcast<u32>(x2) >> 23u) & 255u) - 127i) >> 1u;
  let z = u32(clamp(i32(UNIT_ZOOM) - e, 0i, i32(FINEST_ZOOM)));
  return min(max(z, range.x), range.y);
}

/// How far apart a beam's rays lie at the squared distance `d2` from the walk's origin, given
/// the beam's `spacing`, `across` then `angle` (pxl-engine `cast::Spacing::width`). Only the GPU
/// walks a beam, so this function alone decides the width, for every pass. Its square root may
/// lie a few last places off the Rust's (tests/walk.rs `WIDTH_ULPS`).
fn spacing_width(spacing: vec2<f32>, d2: f32) -> f32 {
  return spacing.x + spacing.y * sqrt(d2);
}

/// Whether a split `side` wide is too narrow for a beam ray to share where the beam's rays lie
/// `width` apart (pxl-engine `cast::narrow`). The bound is pxl-engine's `NARROW_SQUARED`, 8,
/// and not `NARROW` squared in f32, which rounds to one place under 8.
fn narrow(side: f32, width: f32) -> bool {
  return side * side < NARROW_SQUARED * (width * width);
}

/// Where a ray enters a plane it reaches at `t`, no nearer than the origin. At the origin it
/// gives +0, as pxl-engine's `greater(t, 0.0)` does, where `max` may give −0 (cast.rs
/// `entry_plane`).
fn past_origin(t: f32) -> f32 {
  return select(0.0, t, t > 0.0);
}

/// The ray's t at the near and the far plane on each axis (pxl-engine cast.rs `planes`), found
/// without branches. On an axis the ray runs along, the span is the whole of `t` to `t_max`
/// where the ray lies inside the slab, and empty where it lies outside.
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

/// The axis crossed: among the axes the ray moves along, the one whose plane it reaches last
/// where `last`, or first otherwise, the lowest axis at a tie; the third axis where it moves
/// along none (pxl-engine `crossing`).
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

/// Step `e` of a split's box, on the finest grid's own planes (pxl-engine cast.rs `meets_box`,
/// its `step`). On a node `BOX_STEPS` or more finest cells wide a step is a whole count of
/// them. On a finer node the step is rounded out by `out`, the step less one.
fn box_step(e: vec3<u32>, u: u32, out: u32) -> vec3<u32> {
  if (u >= BOX_STEPS - 1u) {
    return e * ((u >> BOX_BITS) + 1u);
  }
  return (e * (u + 1u) + vec3<u32>(out)) >> vec3<u32>(BOX_BITS);
}
