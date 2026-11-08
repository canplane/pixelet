// pxl-render/resume.wgsl

// Where a ray's walk starts, read from what its camera grid cell hands its rays (pxl-engine
// `beam::CELL_WORDS`) and from the line the cell resumes from (`beam::LINE_WORDS`). A line is
// what a beam ray leaves for the pixels around it: the node it stopped at, and what was handed
// each node from the root down to it. The cast's beam rays leave both, the eye's or the light's.

@group(0) @binding(8) var<storage, read_write> beams: array<u32>;

/// The roots of a line's node as a walk from the origin `to` holds them (pxl-engine
/// `cast::Node::moved`). `roots` are the node's roots as a walk from the line's own origin
/// holds them, and that origin is kept at word `k`. The two origins lie within a root of each
/// other, so the roots move by at most one either way.
fn moved_roots(k: u32, roots: vec3<i32>, to: vec3<i32>) -> vec3<i32> {
  let was = vec3<i32>(i32(beams[k]), i32(beams[k + 1u]), i32(beams[k + 2u]));
  let d = was - to;
  let og = vec3<u32>(was) + vec3<u32>(0u, Y_START, Z_START);
  return roots + select(
    select(vec3<i32>(0), vec3<i32>(-1), og + vec3<u32>(-d) < og),
    select(vec3<i32>(0), vec3<i32>(1), og < vec3<u32>(d)),
    d >= vec3<i32>(0),
  );
}

/// Where the ray `aim` starts its walk at the start of the cell at word `at_cell`
/// (`resumed_at`).
fn resumed(aim: Aim, at_cell: u32, line_words: u32, line_origin: u32) -> Start {
  return resumed_at(aim, at_cell, bitcast<f32>(beams[at_cell + CELL_T]), line_words, line_origin);
}

/// Where the ray `aim` starts its walk at `t`, from the line of the cell at word `at_cell`
/// (`line_resumed`). `t` lies at or past the cell's start: it is where the ray's walk over the
/// node cache left it, or the cell's start. Where the cell does not resume from its line, the
/// walk starts at the root at 0. `line_words` is the number of words in a line. `line_origin`
/// is the word within a line where the origin of its nodes lies, or `NONE` where every ray
/// starts at one origin.
fn resumed_at(aim: Aim, at_cell: u32, t: f32, line_words: u32, line_origin: u32) -> Start {
  if (beams[at_cell + CELL_RESUMES] != 1u) {
    return root_start();
  }
  return line_resumed(aim, beams[at_cell + CELL_CORNER] * line_words, t, line_origin);
}

/// Where the ray `aim` starts at `t`, from the line at word `line`: at the deepest node, the
/// line's node or one over it, that holds the ray's point at `t`. Where that point lies in
/// another root than the line's node, the walk starts at the origin's root, still at `t`.
fn line_resumed(aim: Aim, line: u32, t: f32, line_origin: u32) -> Start {
  let dir = ray_dir(aim.dir);
  let inv = aim.inv;
  let og = vec3<u32>(aim.origin) + vec3<u32>(0u, Y_START, Z_START);
  let k = line + LINE_NODE;
  let node_c = vec3<u32>(beams[k + NODE_CORNER], beams[k + NODE_CORNER + 1u], beams[k + NODE_CORNER + 2u]);
  var node_r = vec3<i32>(i32(beams[k + NODE_ROOTS]), i32(beams[k + NODE_ROOTS + 1u]), i32(beams[k + NODE_ROOTS + 2u]));
  if (line_origin != NONE) {
    node_r = moved_roots(line + line_origin, node_r, aim.origin);
  }
  // The ray's point at `t` on the finest grid, and the root it lies in (pxl-engine
  // `cast::point_at`).
  let d = vec3<i32>(floor(dir * t * f32(1u << FRAC_W)));
  let p = og + vec3<u32>(d);
  let on = select(vec3<i32>(0), vec3<i32>(1), (d >= vec3<i32>(0)) & (p < og))
    + select(vec3<i32>(0), vec3<i32>(-1), (d < vec3<i32>(0)) & (p > og));
  if (any(node_r != on)) {
    return start_at(t, vec3<u32>(0u), 0u, vec3<i32>(0));
  }
  let apart_from = (node_c.x ^ p.x) | (node_c.y ^ p.y) | (node_c.z ^ p.z);
  var zoom = min(beams[k + NODE_ZOOM], countLeadingZeros(apart_from));
  var corner = node_c & vec3<u32>(~under(zoom));
  // The point came from a float on the ray and may lie a step outside the node the ray is in,
  // so the start climbs to the node whose planes, as the walk tests them, hold the ray at `t`
  // (pxl-engine `cast::holding`).
  for (var i = 0u; i < 33u && zoom != 0u; i += 1u) {
    let u = under(zoom);
    let s = span_of(dir, inv, units(corner, og, node_r), far_of(corner, u, og, node_r), t, aim.t_max);
    let t0 = max(t, largest(s.near));
    let t1 = min(aim.t_max, smallest(s.far));
    if (t0 <= t && t < t1) {
      break;
    }
    zoom -= 1u;
    corner = corner & vec3<u32>(~under(zoom));
  }
  return start_at(t, corner, zoom, node_r);
}
