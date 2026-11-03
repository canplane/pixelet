// pxl-render/walk.wgsl

// A ray's walk over what the observed pyramid keeps, by hand: no stack, no atomics
// and no query of the world, so a thread holds little across its loop
// (doc/experiment/20261103-the-walk-on-a-phone). A walk that meets a node no slot
// keeps ends left, for the walk that asks the world to do again whole. What it
// reads no thread of it writes.

@group(0) @binding(0) var<uniform> eye: array<vec4<u32>, EYE_VEC4S>;
@group(0) @binding(8) var<storage, read_write> beams: array<u32>;
@group(0) @binding(9) var<storage, read> pyramid: array<u32>;

fn eye_word(k: u32) -> u32 {
  return eye[k / 4u][k % 4u];
}

fn eye_f32(k: u32) -> f32 {
  return bitcast<f32>(eye_word(k));
}

fn eye_v3(k: u32) -> vec3<f32> {
  return vec3<f32>(eye_f32(k), eye_f32(k + 1u), eye_f32(k + 2u));
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

/// The first slot of its bucket (`pyramid::entry_of`).
fn bucket_of(zoom: u32, c: vec3<i32>) -> u32 {
  var h = zoom * 0x85EBCA77u;
  h = mix_hash(h, u32(c.x));
  h = mix_hash(h, u32(c.y));
  h = mix_hash(h, u32(c.z));
  h = h ^ (h >> 16u);
  return h & (ENTRIES - 1u) & ~(BUCKET - 1u);
}

/// Only nodes under the world's roots are kept (`pyramid::keeps`).
fn keeps(zoom: u32, c: vec3<i32>) -> bool {
  if (zoom >= 32u) {
    return false;
  }
  let v = c >> vec3<u32>(zoom);
  return v.x == 0i && (v.y == 0i || v.y == -1i) && (v.z == 0i || v.z == -1i);
}

/// The word of the slot holding the node at the zooms asked; `NONE` where none does.
fn slot_of(zoom: u32, c: vec3<i32>, far: u32, near: u32) -> u32 {
  if (!keeps(zoom, c)) {
    return NONE;
  }
  let want = (far << TAG_BITS) | (near << (TAG_BITS + ZOOM_BITS));
  let first = bucket_of(zoom, c);
  for (var k = 0u; k < BUCKET; k += 1u) {
    let w = ENTRIES_AT + (first + k) * ENTRY_WORDS;
    let held = pyramid[w];
    if (held != 0u && held != CLAIMED && (held & KEY_ZOOMS) == want
      && pyramid[w + W_NODE] == zoom
      && pyramid[w + W_NODE + 1u] == u32(c.x)
      && pyramid[w + W_NODE + 2u] == u32(c.y)
      && pyramid[w + W_NODE + 3u] == u32(c.z)) {
      return w;
    }
  }
  return NONE;
}

/// The word of a slot that holds the node as a split, at any zooms; `NONE` where none
/// does. Asked finer than itself the node is a split or air (`world::node`): a walk
/// that goes in, taking no box of it, meets what its children answer either way. So
/// one node over the eye whose corner crosses a zoom's distance does not send every
/// ray to the world.
fn split_at_other_zooms(zoom: u32, c: vec3<i32>, near: u32) -> u32 {
  if (zoom >= near || !keeps(zoom, c)) {
    return NONE;
  }
  let first = bucket_of(zoom, c);
  for (var k = 0u; k < BUCKET; k += 1u) {
    let w = ENTRIES_AT + (first + k) * ENTRY_WORDS;
    let held = pyramid[w];
    if (held != 0u && held != CLAIMED && pyramid[w + W_KIND] == KIND_SPLIT
      && pyramid[w + W_NODE] == zoom
      && pyramid[w + W_NODE + 1u] == u32(c.x)
      && pyramid[w + W_NODE + 2u] == u32(c.y)
      && pyramid[w + W_NODE + 3u] == u32(c.z)) {
      return w;
    }
  }
  return NONE;
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

/// The side of the box a cell keeping `k` eighths is drawn (`answer::side`).
fn side(k: u32) -> f32 {
  switch (k) {
    case 1u: { return 0.5; }
    case 2u: { return 0.6299605; }
    case 3u: { return 0.7211248; }
    case 4u: { return 0.7937005; }
    case 5u: { return 0.854988; }
    case 6u: { return 0.9085603; }
    case 7u: { return 0.9564656; }
    default: { return 1.0; }
  }
}

/// Step `e` of a box on the grid's own planes, rounded out by `out` where finer
/// than the finest grid's count (`meets_box`'s `step`).
fn box_step(e: vec3<u32>, u: u32, out: u32) -> vec3<u32> {
  if (u >= BOX_STEPS - 1u) {
    return e * ((u >> BOX_BITS) + 1u);
  }
  return (e * (u + 1u) + vec3<u32>(out)) >> vec3<u32>(BOX_BITS);
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
  face: u32,
  t: f32,
  visits: u32,
}

/// The ray at the tangents `tan_at`, from the start of its cell `cell` (`resume`,
/// `resume_walk`, `cast::walk`); or, where `line` is a beam ray's line and not
/// `NONE`, from the root (`Until::Beam`), what was handed each node it visits
/// written there at once, in place of a stack.
fn walk(tan_at: vec2<f32>, cell: vec2<u32>, line: u32) -> Walked {
  let beam = line != NONE;
  let aimed = eye_v3(FORWARD) + tan_at.x * eye_v3(RIGHT) + tan_at.y * eye_v3(UP);
  let raw = aimed / length(aimed);
  let dir = select(raw, vec3<f32>(0.0), abs(raw) < vec3<f32>(1.17549435e-38));
  let inv = select(vec3<f32>(1.0) / dir, vec3<f32>(0.0), dir == vec3<f32>(0.0));
  let span_n = vec2<u32>(eye_word(GRID + 5u), eye_word(GRID + 6u));
  let t_max = eye_f32(FAR);
  let range = vec2<u32>(eye_word(RANGE), eye_word(RANGE + 1u));
  let origin = vec3<i32>(i32(eye_word(ORIGIN)), i32(eye_word(ORIGIN + 1u)), i32(eye_word(ORIGIN + 2u)));
  // On the grid of the root that holds it (`grid_of`, `roots_of`).
  let og = vec3<u32>(origin) + vec3<u32>(0u, Y_START, Z_START);
  let r0 = vec3<i32>(origin.x >> 31u, 0i, i32(u32(origin.z) + Z_START) >> 31u);

  // Where its cell's start hands it (`resume`, `resume_walk`).
  let at_cell = eye_word(GRID + 11u) + (cell.y * span_n.x + cell.x) * CELL_WORDS;
  let resumes = !beam && beams[at_cell + CELL_RESUMES] != 0u;
  var t = 0.0;
  var corner = vec3<u32>(0u);
  var zoom = 0u;
  var roots = vec3<i32>(0);
  var visits = 0u;
  if (resumes) {
    let line = beams[at_cell + CELL_CORNER] * LINE_WORDS;
    visits = beams[line + LINE_VISITS];
    t = bitcast<f32>(beams[at_cell + CELL_T]);
    let k = line + LINE_NODE;
    let node_c = vec3<u32>(beams[k + NODE_CORNER], beams[k + NODE_CORNER + 1u], beams[k + NODE_CORNER + 2u]);
    let node_r = vec3<i32>(i32(beams[k + NODE_ROOTS]), i32(beams[k + NODE_ROOTS + 1u]), i32(beams[k + NODE_ROOTS + 2u]));
    // Where it is at `t`, on the finest grid, and its root (`point_at`).
    let d = vec3<i32>(floor(dir * t * f32(1u << FRAC_W)));
    let p = og + vec3<u32>(d);
    let on = select(vec3<i32>(0), vec3<i32>(1), (d >= vec3<i32>(0)) & (p < og))
      + select(vec3<i32>(0), vec3<i32>(-1), (d < vec3<i32>(0)) & (p > og));
    if (all(node_r == on)) {
      let apart = (node_c.x ^ p.x) | (node_c.y ^ p.y) | (node_c.z ^ p.z);
      zoom = min(beams[k + NODE_ZOOM], countLeadingZeros(apart));
      corner = node_c & vec3<u32>(~under(zoom));
      roots = node_r;
      // The node over it that holds where it is, as the walk's planes say (`holding`).
      for (var i = 0u; i < 33u && zoom != 0u; i += 1u) {
        let u = under(zoom);
        let s = span_of(dir, inv, units(corner, og, roots), far_of(corner, u, og, roots), t, t_max);
        let t0 = max(t, largest(s.near));
        let t1 = min(t_max, smallest(s.far));
        if (t0 <= t && t < t1) {
          break;
        }
        zoom -= 1u;
        corner = corner & vec3<u32>(~under(zoom));
      }
    }
  }

  // 0 goes on; 1 a hit; 2 lacks; 3 gone to its reach; 4 left to `sample_cs`.
  var ends = 0u;
  var asset = vec2<u32>(0u);
  var axis = 0u;
  var hit_t = 0.0;
  var blind = false;
  for (var step = 0u; step < STEPS_MOST && ends == 0u; step += 1u) {
    let u = under(zoom);
    let lo = units(corner, og, roots);
    let hi = far_of(corner, u, og, roots);
    let s = span_of(dir, inv, lo, hi, t, t_max);
    let t0 = max(t, largest(s.near));
    let t1 = min(t_max, smallest(s.far));
    var into = false;
    var visited = false;
    if (t1 > t0) {
      // Its farthest corner and nearest point, squared (`node_dist_sq`).
      let far_v = max(-lo, hi);
      let near_v = min(max(lo, vec3<f32>(0.0)), hi);
      let asked_far = zoom_at(dot(far_v, far_v), range);
      let asked_near = zoom_at(dot(near_v, near_v), range);
      let c = idx_of(corner, zoom, roots, r0);
      var w = slot_of(zoom, c, asked_far, asked_near);
      let other_zooms = w == NONE;
      if (other_zooms) {
        w = split_at_other_zooms(zoom, c, asked_near);
      }
      if (w == NONE) {
        ends = 4u;
        break;
      }
      visits += 1u;
      visited = true;
      if (beam) {
        // Through a split at other zooms, what was found under it is not taken: the
        // next that asks finds it again (`Inheritance::of`), as a moved brick's may
        // be gone.
        let held = line + LINE_INHERITANCE + LINE_STEP * zoom;
        beams[held] = pyramid[w + W_INHERITANCE];
        beams[held + 1u] = select(pyramid[w + W_INHERITANCE + 1u], SHARD_UNKNOWN, other_zooms);
        beams[held + 2u] = select(pyramid[w + W_INHERITANCE + 2u], LOOKUP_NONE, other_zooms);
        beams[held + 3u] = select(pyramid[w + W_INHERITANCE + 3u], SLOT_NOTHING, other_zooms);
      }
      let kind = pyramid[w + W_KIND];
      asset = vec2<u32>(pyramid[w + W_ASSET], pyramid[w + W_ASSET + 1u]);
      var near_at = s.near;
      if (other_zooms) {
        // No box of it read. A beam ray stops at a split too narrow to share, which
        // a box would decide: left.
        let width = eye_f32(GRID) * length(far_v);
        let side_w = hi.x - lo.x;
        if (beam && side_w * side_w < NARROW * NARROW * width * width) {
          ends = 4u;
          break;
        }
        into = true;
        blind = true;
      } else if (kind == KIND_SPLIT) {
        // Whether its span meets the box its content lies in (`meets_box`).
        let m = BOX_STEPS - 1u;
        let first = vec3<u32>(asset.x & m, (asset.x >> (2u * BOX_BITS)) & m, (asset.y >> BOX_BITS) & m);
        let past = vec3<u32>((asset.x >> BOX_BITS) & m, asset.y & m, (asset.y >> (2u * BOX_BITS)) & m) + vec3<u32>(1u);
        // A beam ray's width at its far corner: it stops at a split it may not
        // share with the rays about it, and clears one it clears by that.
        let width = select(0.0, eye_f32(GRID) * length(far_v), beam);
        let side_w = hi.x - lo.x;
        let narrow = beam && side_w * side_w < NARROW * NARROW * width * width;
        let grown = NARROW * width;
        let box_lo = units(corner + box_step(first, u, 0u), og, roots) - vec3<f32>(grown);
        let box_hi = select(
          units(corner + box_step(past, u, BOX_STEPS - 1u), og, roots),
          hi,
          past >= vec3<u32>(BOX_STEPS),
        ) + vec3<f32>(grown);
        let b = span_of(dir, inv, box_lo, box_hi, t0, t_max);
        let meets = max(t0, largest(b.near)) < min(t1, smallest(b.far));
        into = meets && !narrow;
        if (meets && narrow) {
          ends = 1u;
        }
      } else if (kind == KIND_CELL) {
        let v = asset.x & VAL_MASK;
        let clear = v == VALUE_VOID || v == VALUE_AIR;
        let inside = all(lo <= vec3<f32>(0.0)) && all(vec3<f32>(0.0) < hi);
        if (beam) {
          if (!clear) {
            ends = 1u;
          }
        } else if (!clear && !inside) {
          let k = (asset.x >> FILL_SHIFT) & (EIGHTHS - 1u);
          if (k == 0u) {
            ends = 1u;
          } else {
            let inset = (hi - lo) * (0.5 * (1.0 - side(k)));
            let b = span_of(dir, inv, lo + inset, hi - inset, t, t_max);
            near_at = b.near;
            if (min(t_max, smallest(b.far)) > max(t, largest(b.near))) {
              ends = 1u;
            }
          }
        }
      } else {
        // Under a split it took no box of, a walk that read the box may have passed
        // this: left.
        ends = select(2u, 4u, blind);
      }
      if (ends != 0u) {
        axis = crossing(dir, near_at, true);
        hit_t = max(near_at[axis], 0.0);
        break;
      }
    }
    if (into) {
      // PUSH: the child holding where it is at `t0`, past each mid plane crossed.
      let half = (u >> 1u) + 1u;
      let mid = units(corner + vec3<u32>(half), og, roots);
      let crossed = select(
        select(mid <= vec3<f32>(0.0), mid * inv > vec3<f32>(t0), dir < vec3<f32>(0.0)),
        mid * inv <= vec3<f32>(t0),
        dir > vec3<f32>(0.0),
      );
      corner = corner | select(vec3<u32>(0u), vec3<u32>(half), crossed);
      zoom += 1u;
      t = t0;
    } else {
      if (t1 >= t_max) {
        // Unvisited, what was handed it is not in the line: asked of the world.
        ends = select(3u, 4u, beam && !visited);
        hit_t = t1;
        break;
      }
      // ADVANCE: the corner stepped on the axis it leaves by, then POP.
      let e = crossing(dir, s.far, false);
      let up = dir[e] > 0.0;
      let old = corner[e];
      let new_c = select(old - u - 1u, old + u + 1u, up);
      t = max(t, t1);
      if ((up && old + u == 0xffffffffu) || (!up && old == 0u)) {
        roots[e] += select(-1i, 1i, up);
        corner = vec3<u32>(0u);
        zoom = 0u;
      } else {
        corner[e] = new_c;
        let h = 31u - countLeadingZeros(old ^ new_c);
        corner = corner & vec3<u32>(~((1u << h) - 1u));
        zoom = FINEST_ZOOM - h;
      }
    }
  }
  let face = 2u * axis + select(0u, 1u, dir[axis] > 0.0);
  return Walked(ends, zoom, corner, roots, asset, axis, face, hit_t, visits);
}
