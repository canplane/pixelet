// pxl-render/walk.wgsl

// A ray's walk, every walk's one (pxl-engine `cast::walk`): a step a node, no stack, over
// what its kernel's node module answers each node with (`node_answer`): the node cache alone,
// read only (cached.wgsl), or the node cache and the world asked where no slot keeps the node
// (ask.wgsl). What the ray's origin decides is data the walk reads off its `Aim`: where it
// starts (`Start`), the zooms its range holds it to, its stop rule (`UNTIL_OPAQUE`,
// `UNTIL_BEAM`) and a beam ray's line. A node is answered at the zooms asked alone, the pair a
// function of the node and the origin (`cast::ask_zooms`), as pxl-engine's `Query` answers
// it: a walk that meets a node no slot keeps at that pair is left there (`KIND_LEFT`), never
// passed on a slot of other zooms. So a ray walked by either module visits the nodes
// `cast::walk` visits from the same start, in the same order, and stops where it stops. A ray
// walked on from the state a walk returns (`Walked`, `Start`) goes on as one unbroken walk. A
// ray the first module left and the second walks on from its `t` alone (resume.wgsl
// `resumed_at`, `line_resumed`) stops where one unbroken walk stops, as the partition of its
// interval past that point is the same; it looks again at the nodes over the one it was left
// at, and counts its steps afresh.

/// No slot keeps the node at the zooms asked: the walk is left there (cached.wgsl).
const KIND_LEFT = 0xfffffffeu;

/// What was handed the line's node at `zoom`, at `line` (`beam::LINE_INHERITANCE`).
fn line_handed(line: u32, zoom: u32) -> vec4<u32> {
  let k = line + LINE_INHERITANCE + LINE_STEP * zoom;
  return vec4<u32>(beams[k], beams[k + 1u], beams[k + 2u], beams[k + 3u]);
}

fn line_hand(line: u32, zoom: u32, handed: vec4<u32>) {
  let k = line + LINE_INHERITANCE + LINE_STEP * zoom;
  beams[k] = handed.x;
  beams[k + 1u] = handed.y;
  beams[k + 2u] = handed.z;
  beams[k + 3u] = handed.w;
}

/// The ray `aim` from `start`, at most `steps` steps more, to the first cell its stop rule stops
/// at, the first that lacks, its reach, or, for a beam ray, the first split too narrow to share
/// with its beam's rays (`cast::walk_at_most`: at most `STEPS_MOST` steps in all, past which it
/// has gone to its reach). A step is a node looked at; what it carries from one step to the
/// next is `Start`'s words and nothing else, so a walk stopped after any step and walked on
/// from the state it returns (`Walked`) looks at the nodes, and stops where, one walk given
/// both their steps does. A beam ray's line (`aim.line`, `beam::LINE_WORDS`) holds what was
/// handed each node it goes into and the one it ends at, written as it goes, and is read back
/// where the walk pops to a node's child, in place of a stack; and the node it ends or is left
/// at. `ends` (`Walked`): 4 by cached.wgsl, at a node no slot keeps or at the end of a line
/// whose last node's handed words it cannot find; 5 where `steps` ran out first.
fn walk(aim: Aim, start: Start, steps: u32) -> Walked {
  let beam = aim.until == UNTIL_BEAM;
  let line = aim.line;
  let dir = ray_dir(aim.dir);
  let inv = aim.inv;
  let t_max = aim.t_max;
  let range = aim.range;
  // On the grid of the root that holds it (`grid_of`, `roots_of`).
  let og = vec3<u32>(aim.origin) + vec3<u32>(0u, Y_START, Z_START);
  let r0 = root_of(aim.origin);
  var t = start.t;
  var corner = start.corner;
  var zoom = start.zoom;
  var roots = start.roots;
  // What was handed the node's parent, which an asked node's handed words are found off
  // (`world_child`); else, `top`, the node is asked from the top (`world_root`): at a root,
  // and past a pop where no line holds the parent's, as the first after a resume is. A line
  // holds every node's over the one it is at.
  var parent = start.parent;
  var top = start.top;
  if (line != NONE && zoom != 0u) {
    parent = line_handed(line, zoom - 1u);
    top = false;
  }
  let last = start.step + min(steps, STEPS_MOST - min(start.step, STEPS_MOST));
  // 0 goes on.
  var ends = 0u;
  var asset = vec2<u32>(0u);
  var axis = 0u;
  var hit_t = t_max;
  var visits = 0u;
  // Whether what was handed the node it ends at is in the line.
  var handed_ended = false;
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
    var handed = vec4<u32>(0u);
    // Only touched, at a face or an edge: not entered, nothing in it met.
    if (t1 > t0) {
      let zooms = asked_zooms(lo, hi, range);
      let a = node_answer(parent, top, zoom, idx_of(corner, zoom, roots, r0), zooms, line != NONE);
      if (a.kind == KIND_LEFT) {
        ends = 4u;
        hit_t = t;
        break;
      }
      visits += 1u;
      handed = a.handed;
      asset = a.asset;
      let v = asset.x & VAL_MASK;
      let clear = v == VALUE_VOID || v == VALUE_AIR;
      var near_at = s.near;
      if (a.kind == KIND_SPLIT) {
        // Whether its span meets the box its content lies in (`meets_box`), grown for a beam
        // ray by its width at the far corner: it stops at a split it may not share with the
        // rays about it, and clears one it clears by that (`narrow`).
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
          // What the ray starts inside it does not stop at; a cell keeping less than its cube
          // is the box its share fills (`fill_bounds`).
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
        ends = 2u;
        stops = true;
      }
      if (line != NONE && (into || stops || t1 >= t_max)) {
        line_hand(line, zoom, handed);
        handed_ended = !into;
      }
      if (stops) {
        // Where the ray enters the node, or its share's box (`entry_plane`).
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
      parent = handed;
      top = false;
    } else {
      if (t1 >= t_max) {
        ends = 3u;
        hit_t = t1;
        break;
      }
      // ADVANCE: the corner stepped on the axis it leaves by, then POP.
      let e = crossing(dir, s.far, false);
      let up = dir[e] > 0.0;
      let old = corner[e];
      let new_c = select(old - u - 1u, old + u + 1u, up);
      t = max(t, t1);
      handed_ended = false;
      if ((up && old + u == 0xffffffffu) || (!up && old == 0u)) {
        // Past a root: into the next, from its top.
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
          parent = line_handed(line, zoom - 1u);
        }
      }
    }
  }
  if (ends == 0u) {
    // Out of its steps: gone to its reach where they were the last of `STEPS_MOST`, as a float
    // gone wrong ends; else stopped where it is.
    ends = select(5u, 3u, step >= STEPS_MOST);
    hit_t = select(t, t_max, step >= STEPS_MOST);
  }
  if (line != NONE) {
    // The node it ended or was left at, which a walk resumed from the line starts from
    // (resume.wgsl `line_resumed`); and what was handed the node it ended at, where that was
    // not asked.
    let n = line + LINE_NODE;
    beams[n + NODE_ZOOM] = zoom;
    beams[n + NODE_CORNER] = corner.x;
    beams[n + NODE_CORNER + 1u] = corner.y;
    beams[n + NODE_CORNER + 2u] = corner.z;
    beams[n + NODE_ROOTS] = u32(roots.x);
    beams[n + NODE_ROOTS + 1u] = u32(roots.y);
    beams[n + NODE_ROOTS + 2u] = u32(roots.z);
    if (ends <= 3u && !handed_ended && !hand_end(line, parent, top, zoom, idx_of(corner, zoom, roots, r0))) {
      ends = 4u;
    }
  }
  return Walked(ends, zoom, corner, roots, asset, axis, hit_t, visits, parent, top, step);
}

/// The zooms the ray `aim` asks the node at `corner`, `zoom` and `roots` at, as its walk asks
/// them (`asked_zooms`).
fn node_zooms(aim: Aim, corner: vec3<u32>, zoom: u32, roots: vec3<i32>) -> u32 {
  let og = vec3<u32>(aim.origin) + vec3<u32>(0u, Y_START, Z_START);
  return asked_zooms(units(corner, og, roots), far_of(corner, under(zoom), og, roots), aim.range);
}
