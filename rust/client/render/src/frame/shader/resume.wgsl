// pxl-render/resume.wgsl

// Where a ray's walk starts off its cell (pxl-engine `beam::CELL_WORDS`, `beam::LINE_WORDS`):
// the words a cast's beam rays left, the eye's or the light's.

@group(0) @binding(8) var<storage, read_write> beams: array<u32>;

/// Where a line's node is held from another ray's origin, kept at word `k`: its roots as a walk
/// from `to` holds them (`Node::moved`).
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

/// The ray `aim` at the start of the cell at word `at_cell`, off the deepest node of its line
/// that holds where it is there, or the root at it where that lies in another root; from the
/// root at 0 where it does not resume. `line_words` a line's words, `line_origin` where in a line
/// the origin its nodes are held from lies, `NONE` where every ray starts at one.
fn resumed(aim: Aim, at_cell: u32, line_words: u32, line_origin: u32) -> Start {
  if (beams[at_cell + CELL_RESUMES] == 0u) {
    return root_start();
  }
  let dir = ray_dir(aim.dir);
  let inv = ray_inv(dir);
  let og = vec3<u32>(aim.origin) + vec3<u32>(0u, Y_START, Z_START);
  let line = beams[at_cell + CELL_CORNER] * line_words;
  let visits = beams[line + LINE_VISITS];
  let t = bitcast<f32>(beams[at_cell + CELL_T]);
  let k = line + LINE_NODE;
  let node_c = vec3<u32>(beams[k + NODE_CORNER], beams[k + NODE_CORNER + 1u], beams[k + NODE_CORNER + 2u]);
  var node_r = vec3<i32>(i32(beams[k + NODE_ROOTS]), i32(beams[k + NODE_ROOTS + 1u]), i32(beams[k + NODE_ROOTS + 2u]));
  if (line_origin != NONE) {
    node_r = moved_roots(line + line_origin, node_r, aim.origin);
  }
  // Where it is at `t`, on the finest grid, and its root (`point_at`).
  let d = vec3<i32>(floor(dir * t * f32(1u << FRAC_W)));
  let p = og + vec3<u32>(d);
  let on = select(vec3<i32>(0), vec3<i32>(1), (d >= vec3<i32>(0)) & (p < og))
    + select(vec3<i32>(0), vec3<i32>(-1), (d < vec3<i32>(0)) & (p > og));
  if (any(node_r != on)) {
    return Start(t, vec3<u32>(0u), 0u, vec3<i32>(0), visits);
  }
  let apart_from = (node_c.x ^ p.x) | (node_c.y ^ p.y) | (node_c.z ^ p.z);
  var zoom = min(beams[k + NODE_ZOOM], countLeadingZeros(apart_from));
  var corner = node_c & vec3<u32>(~under(zoom));
  // The node over it that holds where it is, as the walk's planes say (`holding`).
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
  return Start(t, corner, zoom, node_r, visits);
}
