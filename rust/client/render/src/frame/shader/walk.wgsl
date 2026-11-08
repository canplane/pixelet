// pxl-render/walk.wgsl

// A ray's walk, the one every walk runs (pxl-engine `cast::walk`): one node a step, with no
// stack. The kernel's node module answers each node (`node_answer`), either from the node
// cache alone, read only (cached.wgsl), or from the node cache with the world asked where no
// slot keeps the node (ask.wgsl). What the ray's origin decides is data the walk reads from its
// `Aim`: where it starts (`Start`), the zooms its range holds it to, its stop rule
// (`UNTIL_OPAQUE`, `UNTIL_BEAM`) and a beam ray's line.
//
// A node is answered at the zooms asked, a pair that depends on the node and the origin alone
// (pxl-engine `cast::ask_zooms`), as pxl-engine's `Query` answers it. A walk that meets a node
// no slot keeps at that pair is left there (`KIND_LEFT`). The exception is a split that a slot
// keeps at other zooms, or forgot: the walk goes into it without taking its box (`KIND_BLIND`).
// A split asked finer than itself is a split or air, and its children answer the ray either
// way. Past such a split the walk may meet a lack that the box would have kept it from, so the
// node cache's walk stops at no lack, and the walk that asks walks that ray again whole
// (`lack_ends`). So a ray stops where `cast::walk` stops.
//
// A ray walked on from the state a walk returns (`WalkEnd`, `Start`) goes on as one unbroken
// walk. A ray that the first module left and the second walks on from its `t` alone
// (resume.wgsl `resumed_at`, `line_resumed`) also stops where one unbroken walk stops, because
// its interval past that point is partitioned the same way. It looks again at the nodes over
// the one it was left at, and counts its steps afresh.

/// The answer where no slot keeps the node at the zooms asked, so the walk is left there
/// (cached.wgsl).
const KIND_LEFT = 0xfffffffeu;
/// The answer where a slot keeps the node as a split at other zooms, or forgot it, so the walk
/// goes into it without its box (cached.wgsl).
const KIND_BLIND = 0xfffffffdu;
/// How a walk ends at a lack where only the walk that asks may stop at one: its ray is walked
/// again whole (`lack_ends`, queue_walk.wgsl).
const ENDS_AGAIN = 6u;

/// What was handed the node at `zoom` of the line at word `line` (pxl-engine
/// `beam::LINE_INHERITANCE`).
fn line_inheritance(line: u32, zoom: u32) -> vec4<u32> {
  let k = line + LINE_INHERITANCE + LINE_STEP * zoom;
  return vec4<u32>(beams[k], beams[k + 1u], beams[k + 2u], beams[k + 3u]);
}

fn line_hand(line: u32, zoom: u32, inheritance: vec4<u32>) {
  let k = line + LINE_INHERITANCE + LINE_STEP * zoom;
  beams[k] = inheritance.x;
  beams[k + 1u] = inheritance.y;
  beams[k + 2u] = inheritance.z;
  beams[k + 3u] = inheritance.w;
}

/// Walks the ray `aim` from `start` for at most `steps` more steps (pxl-engine
/// `cast::walk_at_most`). The walk goes to the first cell its stop rule stops at, the first node
/// that lacks, its reach, or, for a beam ray, the first split too narrow to share with its
/// beam's rays. A ray that has taken `STEPS_CAP` steps in all counts as gone to its reach. A
/// step is one node looked at, and only `Start`'s words carry from one step to the next. So a
/// walk stopped after any step and walked on from the state it returns (`WalkEnd`) looks at the
/// same nodes, and stops at the same place, as one walk given both their steps.
///
/// A beam ray's line (`aim.line`, `beam::LINE_WORDS`) takes the place of a stack. The walk
/// writes into it what was handed each node it goes into and the node it ends at, and reads that
/// back after a pop, for the parent of the node it moves to. At the end the line also takes the
/// node the walk ended or was left at. `ends` is 4 where cached.wgsl leaves the walk, at a node
/// no slot keeps or at the end of a line where it cannot find what was handed the last node,
/// and 5 where `steps` ran out first.
fn walk(aim: Aim, start: Start, steps: u32) -> WalkEnd {
  let beam = aim.until == UNTIL_BEAM;
  let line = aim.line;
  let dir = ray_dir(aim.dir);
  let inv = aim.inv;
  let t_max = aim.t_max;
  let range = aim.range;
  // The origin on the grid of the root that holds it, and that root (pxl-engine
  // `cast::grid_of`, `roots_of`).
  let og = vec3<u32>(aim.origin) + vec3<u32>(0u, Y_START, Z_START);
  let r0 = root_of(aim.origin);
  var t = start.t;
  var corner = start.corner;
  var zoom = start.zoom;
  var roots = start.roots;
  // What was handed the node's parent, from which an asked node's handed words are found
  // (`world_child`). Where `top` is set, the node is asked from the top instead (`world_root`):
  // at a root, and after a pop where no line holds the parent's words, as at the first node
  // after a resume. A line holds the words of every node over the one the walk is at.
  var parent = start.parent;
  var top = start.top;
  if (line != NONE && zoom != 0u) {
    parent = line_inheritance(line, zoom - 1u);
    top = false;
  }
  let last = start.step + min(steps, STEPS_CAP - min(start.step, STEPS_CAP));
  // `ends` stays 0 while the walk goes on.
  var ends = 0u;
  var asset = vec2<u32>(0u);
  var axis = 0u;
  var hit_t = t_max;
  var visits = 0u;
  // Whether the line holds what was handed the node the walk ends at.
  var end_in_line = false;
  var step = start.step;
  for (; step < last; step += 1u) {
    let u = under(zoom);
    let lo = units(corner, og, roots);
    let hi = far_of(corner, u, og, roots);
    let s = span_of(dir, inv, lo, hi, t, t_max);
    let t0 = max(t, largest(s.near));
    let t1 = min(t_max, smallest(s.far));
    var into = false;
    var stops = false;
    var inheritance = vec4<u32>(0u);
    // A node the ray only touches, at a face or an edge, is not entered, and nothing in it is
    // met.
    if (t1 > t0) {
      let zooms = asked_zooms(lo, hi, range);
      let a = node_answer(parent, top, zoom, idx_of(corner, zoom, roots, r0), zooms, line != NONE);
      if (a.kind == KIND_LEFT) {
        ends = 4u;
        hit_t = t;
        break;
      }
      visits += 1u;
      inheritance = a.inheritance;
      asset = a.asset;
      let v = asset.x & VAL_MASK;
      let clear = v == VALUE_VOID || v == VALUE_AIR;
      var near_at = s.near;
      if (a.kind == KIND_BLIND) {
        // A beam ray stops at a split too narrow to share where it meets the split's box, and
        // without the box that cannot be decided, so the walk is left here.
        if (beam && narrow(hi.x - lo.x, spacing_width(aim.spacing, squared(max(-lo, hi))))) {
          ends = 4u;
          hit_t = t;
          break;
        }
        into = true;
      } else if (a.kind == KIND_SPLIT) {
        // Whether the ray's span meets the box the split's content lies in (pxl-engine
        // `meets_box`). For a beam ray the box is grown by the beam's width at the far corner:
        // the ray stops at a split it meets and may not share with the rays around it, and
        // passes a split it clears by that width (`narrow`).
        let width = select(0.0, spacing_width(aim.spacing, squared(max(-lo, hi))), beam);
        let too_narrow = beam && narrow(hi.x - lo.x, width);
        let grown = NARROW * width;
        let past = box_past(asset);
        let box_lo = units(corner + box_step(box_first(asset), u, 0u), og, roots) - vec3<f32>(grown);
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
          stops = true;
        }
      } else if (a.kind == KIND_CELL) {
        let inside = all(lo <= vec3<f32>(0.0)) && all(vec3<f32>(0.0) < hi);
        if (beam) {
          if (!clear) {
            ends = 1u;
            stops = true;
          }
        } else if (!clear && !inside) {
          // A ray does not stop at the cell it starts inside. A cell that keeps less than its
          // cube is the box its share fills (pxl-engine `fill_bounds`).
          let k = (asset.x >> FILL_SHIFT) & (EIGHTHS - 1u);
          if (k == 0u) {
            ends = 1u;
            stops = true;
          } else {
            let inset = (hi - lo) * (0.5 * (1.0 - side(k)));
            let b = span_of(dir, inv, lo + inset, hi - inset, t, t_max);
            if (min(t_max, smallest(b.far)) > max(t, largest(b.near))) {
              near_at = b.near;
              ends = 1u;
              stops = true;
            }
          }
        }
      } else {
        ends = lack_ends();
        stops = true;
      }
      if (line != NONE && (into || stops || t1 >= t_max)) {
        line_hand(line, zoom, inheritance);
        end_in_line = !into;
      }
      if (stops) {
        // Where the ray enters the node, or the box of its share (pxl-engine `entry_plane`).
        axis = crossing(dir, near_at, true);
        hit_t = past_origin(near_at[axis]);
        break;
      }
    }
    if (into) {
      // PUSH: into the child that holds the ray's point at `t0`, on the far side of each middle
      // plane the ray has crossed.
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
      parent = inheritance;
      top = false;
    } else {
      if (t1 >= t_max) {
        ends = 3u;
        hit_t = t1;
        break;
      }
      // ADVANCE: the corner steps on the axis the ray leaves by. Then POP: up to the largest
      // node that holds the new corner and not the old.
      let e = crossing(dir, s.far, false);
      let up = dir[e] > 0.0;
      let old = corner[e];
      let new_c = select(old - u - 1u, old + u + 1u, up);
      t = max(t, t1);
      end_in_line = false;
      if ((up && old + u == 0xffffffffu) || (!up && old == 0u)) {
        // Past a root's edge the walk goes into the next root, from its top.
        roots[e] += select(-1i, 1i, up);
        corner = vec3<u32>(0u);
        zoom = 0u;
        top = true;
      } else {
        corner[e] = new_c;
        let h = 31u - countLeadingZeros(old ^ new_c);
        corner = corner & vec3<u32>(~((1u << h) - 1u));
        zoom = FINEST_ZOOM - h;
        top = line == NONE;
        if (!top) {
          parent = line_inheritance(line, zoom - 1u);
        }
      }
    }
  }
  if (ends == 0u) {
    // The walk ran out of steps. Where it has taken `STEPS_CAP` steps in all it counts as gone
    // to its reach, as a walk a float sent wrong ends; otherwise it stops where it is.
    ends = select(5u, 3u, step >= STEPS_CAP);
    hit_t = select(t, t_max, step >= STEPS_CAP);
  }
  if (line != NONE) {
    // The line takes the node the walk ended or was left at, which a walk resumed from the line
    // starts from (resume.wgsl `line_resumed`). Where the walk did not ask what was handed that
    // node, `hand_end` finds it, or else the walk is left there.
    let n = line + LINE_NODE;
    beams[n + NODE_ZOOM] = zoom;
    beams[n + NODE_CORNER] = corner.x;
    beams[n + NODE_CORNER + 1u] = corner.y;
    beams[n + NODE_CORNER + 2u] = corner.z;
    beams[n + NODE_ROOTS] = u32(roots.x);
    beams[n + NODE_ROOTS + 1u] = u32(roots.y);
    beams[n + NODE_ROOTS + 2u] = u32(roots.z);
    if (ends <= 3u && !end_in_line && !hand_end(line, parent, top, zoom, idx_of(corner, zoom, roots, r0))) {
      ends = 4u;
    }
  }
  return WalkEnd(ends, zoom, corner, roots, asset, axis, hit_t, visits, parent, top, step);
}

/// The walk `w` ended as a lack at the deepest node it held, its ray going no further.
fn to_lack(w: WalkEnd) -> WalkEnd {
  return WalkEnd(2u, w.zoom, w.corner, w.roots, w.asset, w.axis, w.t, w.visits, w.parent, w.top, w.step);
}

/// The zooms at which the ray `aim` asks the node at `corner`, `zoom` and `roots`, as its walk
/// asks them (`asked_zooms`).
fn node_zooms(aim: Aim, corner: vec3<u32>, zoom: u32, roots: vec3<i32>) -> u32 {
  let og = vec3<u32>(aim.origin) + vec3<u32>(0u, Y_START, Z_START);
  return asked_zooms(units(corner, og, roots), far_of(corner, under(zoom), og, roots), aim.range);
}
