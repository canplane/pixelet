// pxl-render/beam_walk.wgsl

// A beam ray walked from the world's root to the first cell that fills or lacks, or the first
// split too narrow to share (`cast::walk`, `Until::Beam`), through the observed pyramid writing
// what it asks (ask.wgsl), either observer's (beam.wgsl, light_beam.wgsl). Its line is left as
// it goes (pxl-engine `beam::LINE_WORDS`): what was handed each node it goes into is written at
// that node's zoom, read back where the walk pops to its child, in place of a stack.

/// What was handed the line's node at `zoom`, at `base` (`beam::LINE_INHERITANCE`).
fn line_handed(base: u32, zoom: u32) -> vec4<u32> {
  let k = base + LINE_INHERITANCE + LINE_STEP * zoom;
  return vec4<u32>(beams[k], beams[k + 1u], beams[k + 2u], beams[k + 3u]);
}

fn line_hand(base: u32, zoom: u32, handed: vec4<u32>) {
  let k = base + LINE_INHERITANCE + LINE_STEP * zoom;
  beams[k] = handed.x;
  beams[k + 1u] = handed.y;
  beams[k + 2u] = handed.z;
  beams[k + 3u] = handed.w;
}

/// Where a beam ray stopped, or went on to its reach, and the nodes it asked.
struct Beamed {
  t: f32,
  visits: u32,
}

/// The beam ray from `origin` aimed `aimed`, its reciprocal `inv` (walk_node.wgsl `Aim`), reaching `t_max` at the zooms `range` holds it to,
/// its beam's rays `spacing` apart, `across` then `angle` (`cast::Spacing`), its line left at
/// `base` (`cast::walk_from_root`, `Until::Beam`; the line's words `beam::LINE_WORDS`).
fn beam_walk(origin: vec3<i32>, aimed: vec3<f32>, inv: vec3<f32>, t_max: f32, range: vec2<u32>, spacing: vec2<f32>, base: u32) -> Beamed {
  let dir = ray_dir(aimed);
  // On the grid of the root that holds it (`grid_of`, `roots_of`).
  let og = vec3<u32>(origin) + vec3<u32>(0u, Y_START, Z_START);
  let r0 = vec3<i32>(origin.x >> 31u, 0i, i32(u32(origin.z) + Z_START) >> 31u);
  var t = 0.0;
  var corner = vec3<u32>(0u);
  var zoom = 0u;
  var roots = vec3<i32>(0);
  var at = world_root(0u, idx_of(corner, zoom, roots, r0));
  var visits = 0u;
  // A walk that runs out of steps, a node looked at a step, has gone to its reach, as
  // `cast::walk_at_most` and `walk_asking` end one: it hands on `t_max`, as a ray that reached
  // it does, its line left where it is.
  var hit_t = t_max;
  for (var step = 0u; step < STEPS_MOST; step += 1u) {
    let u = under(zoom);
    let lo = units(corner, og, roots);
    let hi = far_of(corner, u, og, roots);
    let s = span_of(dir, inv, lo, hi, t, t_max);
    let t0 = max(t, largest(s.near));
    let t1 = min(t_max, smallest(s.far));
    // Only touched, at a face or an edge: not entered, nothing in it met.
    var into = false;
    var ends = false;
    if (t1 > t0) {
      // The zooms it asks, off its farthest corner and nearest point (`ask_zooms`,
      // `node_dist_sq`).
      let far_v = max(-lo, hi);
      let near_v = min(max(lo, vec3<f32>(0.0)), hi);
      let far2 = squared(far_v);
      let zooms = zoom_at(far2, range) | (zoom_at(squared(near_v), range) << ZOOM_BITS);
      let a = ask_handed(at, zoom, idx_of(corner, zoom, roots, r0), zooms, false);
      at = a.handed;
      visits += 1u;
      if (a.kind == KIND_LACKS) {
        ends = true;
      } else if (a.kind == KIND_CELL) {
        let v = a.asset.x & VAL_MASK;
        ends = v != VALUE_VOID && v != VALUE_AIR;
      } else if (a.kind == KIND_SPLIT) {
        // It passes whole a split whose box it clears by `2√2` of the spacing, stops at
        // one too narrow to share with the rays about it, and enters every other
        // (`narrow`, `meets_box`).
        let width = spacing_width(spacing, far2);
        let too_narrow = narrow(hi.x - lo.x, width);
        let grown = NARROW * width;
        let past = box_past(a.asset);
        let box_lo = units(corner + box_step(box_first(a.asset), u, 0u), og, roots) - vec3<f32>(grown);
        let box_hi = select(
          units(corner + box_step(past, u, BOX_STEPS - 1u), og, roots),
          hi,
          past >= vec3<u32>(BOX_STEPS),
        ) + vec3<f32>(grown);
        let b = span_of(dir, inv, box_lo, box_hi, t0, t_max);
        let meets = max(t0, largest(b.near)) < min(t1, smallest(b.far));
        into = meets && !too_narrow;
        ends = meets && too_narrow;
      }
      if (ends) {
        // Where the ray enters the node (`entry_plane`).
        let axis = crossing(dir, s.near, true);
        hit_t = past_origin(s.near[axis]);
        break;
      }
    }
    if (into) {
      // PUSH: the child holding where it is at `t0`, past each mid plane crossed.
      line_hand(base, zoom, at);
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
      at = world_child(at, zoom, idx_of(corner, zoom, roots, r0));
    } else {
      if (t1 >= t_max) {
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
        // Past a root: into the next, from its top.
        roots[e] += select(-1i, 1i, up);
        corner = vec3<u32>(0u);
        zoom = 0u;
        at = world_root(0u, idx_of(corner, zoom, roots, r0));
      } else {
        corner[e] = new_c;
        let h = 31u - countLeadingZeros(old ^ new_c);
        corner = corner & vec3<u32>(~((1u << h) - 1u));
        zoom = FINEST_ZOOM - h;
        at = world_child(line_handed(base, zoom - 1u), zoom, idx_of(corner, zoom, roots, r0));
      }
    }
  }
  // The node it ended at and what was handed it; every node over it holds its own already.
  let n = base + LINE_NODE;
  beams[n + NODE_ZOOM] = zoom;
  beams[n + NODE_CORNER] = corner.x;
  beams[n + NODE_CORNER + 1u] = corner.y;
  beams[n + NODE_CORNER + 2u] = corner.z;
  beams[n + NODE_ROOTS] = u32(roots.x);
  beams[n + NODE_ROOTS + 1u] = u32(roots.y);
  beams[n + NODE_ROOTS + 2u] = u32(roots.z);
  line_hand(base, zoom, at);
  return Beamed(hit_t, visits);
}
