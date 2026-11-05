// pxl-render/walk.wgsl

// A ray's walk over what the node cache keeps, by hand: no stack, no atomics
// and no query of the world, so a thread holds little across its loop
// (doc/experiment/20261103-the-walk-on-a-phone). A walk that meets a node no slot
// keeps ends left, for the walk that asks the world to do again whole. What it
// reads no thread of it writes.

@group(0) @binding(9) var<storage, read> node_cache: array<u32>;

/// The slot a walk takes for a node, its kind and asset; `w` `NONE` where none answers.
struct Slot {
  w: u32,
  /// Held at other zooms than asked.
  other: bool,
  kind: u32,
  asset: vec2<u32>,
}

/// The slot holding the node at the zooms asked; else the first that holds it at other zooms
/// and answers for the zooms asked. A split asked finer than itself is a split or air
/// (`world::node`): a walk that goes in, taking no box of it, meets what its children answer
/// either way. Air held no finer than asked stays air on nearly every node, and a ray goes past
/// it (`revisits`). One scan of the bucket: each entry's key and node read together, its kind
/// and asset only where it holds the node.
fn slot_of(zoom: u32, c: vec3<i32>, far: u32, near: u32) -> Slot {
  // The first at other zooms, held apart: no function's local is a struct (build.rs).
  var other = NONE;
  var other_kind = 0u;
  var other_asset = vec2<u32>(0u);
  if (!keeps(zoom, c)) {
    return Slot(NONE, true, 0u, other_asset);
  }
  let want = (far << TAG_BITS) | (near << (TAG_BITS + ZOOM_BITS));
  let node = vec4<u32>(zoom, u32(c.x), u32(c.y), u32(c.z));
  let first = bucket_of(zoom, c);
  for (var k = 0u; k < BUCKET; k += 1u) {
    let w = ENTRIES_AT + (first + k) * ENTRY_WORDS;
    let held = node_cache[w];
    let n = vec4<u32>(node_cache[w + W_NODE], node_cache[w + W_NODE + 1u], node_cache[w + W_NODE + 2u], node_cache[w + W_NODE + 3u]);
    if (held == 0u || held == CLAIMED || any(n != node)) {
      continue;
    }
    let kind = node_cache[w + W_KIND] & KIND_MASK;
    let asset = vec2<u32>(node_cache[w + W_ASSET], node_cache[w + W_ASSET + 1u]);
    if ((held & KEY_ZOOMS) == want) {
      return Slot(w, false, kind, asset);
    }
    if (other == NONE) {
      let v = asset.x & VAL_MASK;
      let air = kind == KIND_CELL && (v == VALUE_VOID || v == VALUE_AIR);
      let split = kind == KIND_SPLIT && zoom < near;
      if (split || (air && (held >> (TAG_BITS + ZOOM_BITS)) <= near)) {
        other = w;
        other_kind = kind;
        other_asset = asset;
      }
    }
  }
  return Slot(other, true, other_kind, other_asset);
}

/// Whether this ray asks again of a node it could go past, one ray in `REVISIT_ONE_IN`, other
/// rays on other frames: what it asks is kept at the zooms asked, for the next frame's.
fn revisits(seed: u32) -> bool {
  return (seed >> 8u) % REVISIT_ONE_IN == 0u;
}

/// What was handed the node at `zoom` off slot `w`, written in the beam ray's line at `line`.
/// Through a node at other zooms, what was found under it is not taken: the next that asks
/// finds it again (`Inheritance::of`), as a moved brick's may be gone.
fn hand(line: u32, zoom: u32, w: u32, other_zooms: bool) {
  let held = line + LINE_INHERITANCE + LINE_STEP * zoom;
  beams[held] = node_cache[w + W_INHERITANCE];
  beams[held + 1u] = select(node_cache[w + W_INHERITANCE + 1u], SHARD_UNKNOWN, other_zooms);
  beams[held + 2u] = select(node_cache[w + W_INHERITANCE + 2u], LOOKUP_NONE, other_zooms);
  beams[held + 3u] = select(node_cache[w + W_INHERITANCE + 3u], SLOT_NOTHING, other_zooms);
}

/// The ray `aim`, from `start`; or, where `line` is a beam ray's line and not `NONE`, from the
/// root (`Until::Beam`), its beam's rays `spacing` apart (`cast::Spacing`), what was handed each
/// node it goes into and the one it ends at written there at once, in place of a stack.
fn walk(aim: Aim, start: Start, line: u32, spacing: vec2<f32>) -> Walked {
  let beam = line != NONE;
  let dir = ray_dir(aim.dir);
  let inv = aim.inv;
  let t_max = aim.t_max;
  let range = aim.range;
  let origin = aim.origin;
  // On the grid of the root that holds it (`grid_of`, `roots_of`).
  let og = vec3<u32>(origin) + vec3<u32>(0u, Y_START, Z_START);
  let r0 = vec3<i32>(origin.x >> 31u, 0i, i32(u32(origin.z) + Z_START) >> 31u);
  var t = start.t;
  var corner = start.corner;
  var zoom = start.zoom;
  var roots = start.roots;
  var visits = start.visits;

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
      // The zooms asked at its farthest corner and nearest point, squared (`node_dist_sq`);
      // one range's alone, as the light's is.
      var asked_far = range.x;
      var asked_near = range.x;
      if (range.x != range.y) {
        asked_far = zoom_at(squared(max(-lo, hi)), range);
        asked_near = zoom_at(squared(min(max(lo, vec3<f32>(0.0)), hi)), range);
      }
      let c = idx_of(corner, zoom, roots, r0);
      let slot = slot_of(zoom, c, asked_far, asked_near);
      let w = slot.w;
      let other_zooms = slot.other;
      if (w == NONE || (other_zooms && slot.kind == KIND_CELL && revisits(aim.seed))) {
        ends = 4u;
        hit_t = t;
        break;
      }
      visits += 1u;
      visited = true;
      let kind = slot.kind;
      asset = slot.asset;
      var near_at = s.near;
      if (other_zooms && kind == KIND_CELL) {
        // Air: gone past.
      } else if (other_zooms) {
        // No box of it read. A beam ray stops at a split too narrow to share, which
        // a box would decide: left.
        let width = spacing_width(spacing, squared(max(-lo, hi)));
        if (beam && narrow(hi.x - lo.x, width)) {
          ends = 4u;
          hit_t = t;
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
        let width = select(0.0, spacing_width(spacing, squared(max(-lo, hi))), beam);
        let too_narrow = beam && narrow(hi.x - lo.x, width);
        let grown = NARROW * width;
        let box_lo = units(corner + box_step(first, u, 0u), og, roots) - vec3<f32>(grown);
        let box_hi = select(
          units(corner + box_step(past, u, BOX_STEPS - 1u), og, roots),
          hi,
          past >= vec3<u32>(BOX_STEPS),
        ) + vec3<f32>(grown);
        let b = span_of(dir, inv, box_lo, box_hi, t0, t_max);
        let meets = max(t0, largest(b.near)) < min(t1, smallest(b.far));
        into = meets && !too_narrow;
        if (meets && too_narrow) {
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
      // The line holds what was handed the nodes it goes into and the one it ends at, which a
      // walk resumed from it and a hit's faces read; one it is left at, `beam_cs` writes.
      if (beam && (into || ends == 1u || ends == 2u || t1 >= t_max)) {
        hand(line, zoom, w, other_zooms);
      }
      if (ends != 0u) {
        axis = crossing(dir, near_at, true);
        hit_t = past_origin(near_at[axis]);
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
  // Out of steps, it is left to the walk that asks, as one that meets a node no slot keeps.
  if (ends == 0u) {
    ends = 4u;
    hit_t = t;
  }
  return Walked(ends, zoom, corner, roots, asset, axis, hit_t, visits);
}
