// pxl-render/face.wgsl

// What lies about the face each hit's ray entered, asked of the world (world.wgsl) once a hit,
// drawable entries' voxels and the missed pixels' alike, over the args the pass is bound to (pxl-engine
// `face::word`, `support::sub_line`, `support::asset`). Each cell beside the hit is asked from
// the deepest node it shares with the line down to the hit, that line read where the hit's
// cell's beam ray left it over the node they share and handed down from there under it.

@group(0) @binding(8) var<storage, read_write> beams: array<u32>;
/// The node cache, whose drawable entries' faces' words an ask names (pxl-engine
/// `record::TO_RECORD`, `node_cache::W_FACES`): an ask a face, so one writer a word.
@group(0) @binding(9) var<storage, read_write> node_cache: array<u32>;
/// Where its asks lie and how many (`raster::ARGS_ASKS`, `raster::ARGS_INSTANCES`).
@group(0) @binding(16) var<storage, read> args: array<u32>;

/// The node over node `c` of `zoom` at `z`, `z` no finer (`overwrite::ancestor_at`).
fn over(c: vec3<i32>, zoom: u32, z: u32) -> vec3<i32> {
  let steps = zoom - z;
  let p = path(zoom, c);
  return vec3<i32>(c.x >> steps, (p.y >> steps) - y_start(z), (p.z >> steps) - z_start(z));
}

/// The deepest zoom at which node `a` of `za` and node `c` of `zoom` share a node, no finer
/// than either, and whether they share one at all, 1 where they do: none in two roots
/// (`support::shared_zoom`).
fn shared_zoom(a: vec3<i32>, za: u32, c: vec3<i32>, zoom: u32) -> vec2<u32> {
  var z = min(zoom, za);
  for (var i = 0u; i < 33u && z > 0u && any(over(a, za, z) != over(c, zoom, z)); i += 1u) {
    z -= 1u;
  }
  return vec2<u32>(z, select(0u, 1u, all(over(a, za, z) == over(c, zoom, z))));
}

/// What was handed the node at `z` of the beam ray's line at word `line` (pxl-engine
/// `beam::LINE_INHERITANCE`, `beam::LINE_STEP`).
fn line_handed(line: u32, z: u32) -> vec4<u32> {
  let k = line + LINE_INHERITANCE + LINE_STEP * z;
  return vec4<u32>(beams[k], beams[k + 1u], beams[k + 2u], beams[k + 3u]);
}

/// The node at word `k`, as a line and a sample each hold one, by its index from the eye
/// (`Node::idx`), and its zoom in `w`.
fn node_idx(k: u32) -> vec4<i32> {
  let o = vec3<i32>(i32(eye_word(ORIGIN)), i32(eye_word(ORIGIN + 1u)), i32(eye_word(ORIGIN + 2u)));
  let r0 = vec3<i32>(o.x >> 31u, 0i, i32(u32(o.z) + Z_START) >> 31u);
  let zoom = beams[k + NODE_ZOOM];
  let corner = vec3<u32>(beams[k + NODE_CORNER], beams[k + NODE_CORNER + 1u], beams[k + NODE_CORNER + 2u]);
  let roots = vec3<i32>(i32(beams[k + NODE_ROOTS]), i32(beams[k + NODE_ROOTS + 1u]), i32(beams[k + NODE_ROOTS + 2u]));
  return vec4<i32>(idx_of(corner, zoom, roots, r0), i32(zoom));
}

/// The hit's own descent from where it leaves the line (`support::sub_line`): what was handed
/// its nodes at the two zooms over the hit, `up`'s, each the descent's first where that lies no
/// deeper (`descent`).
struct Descent {
  up: vec2<u32>,
  grand_handed: vec4<u32>,
  parent_handed: vec4<u32>,
}

/// The hit `c` at `zoom`'s descent from `low`, handed `low_handed` there, each node under handed
/// down by its parent, worked out once for every cell beside it (`hides`).
fn descent(c: vec3<i32>, zoom: u32, low: u32, low_handed: vec4<u32>) -> Descent {
  var up = vec2<u32>(low);
  var grand = low_handed;
  var parent = low_handed;
  var at = low_handed;
  var z = low + 1u;
  for (var i = 0u; i < 33u && z < zoom; i += 1u) {
    at = world_child(at, z, over(c, zoom, z));
    if (z + 2u == zoom) {
      up.x = z;
      grand = at;
    }
    if (z + 1u == zoom) {
      up.y = z;
      parent = at;
    }
    z += 1u;
  }
  return Descent(up, grand, parent);
}

/// Whether the cell `n` at `zoom` beside the hit hides its cube whole (`answer::whole`), as
/// the world answers it at that zoom, asked off the line down to the hit `c`: over `low` the
/// beam ray's line at word `line`, from there the hit's own descent, handed `low_handed` at
/// `low` and `grand_handed`, `parent_handed` at `up` (`descent`; `support::sub_line`,
/// `JoinedLine`). From the deepest node of that line holding it, each node under handed down by
/// its parent; from its own root where none does (`support::asset`).
fn hides(
  line: u32,
  low: u32,
  low_handed: vec4<u32>,
  up: vec2<u32>,
  grand_handed: vec4<u32>,
  parent_handed: vec4<u32>,
  c: vec3<i32>,
  zoom: u32,
  n: vec3<i32>,
) -> bool {
  let meet = shared_zoom(c, zoom, n, zoom);
  var z = 0u;
  var at = world_root(0u, over(n, zoom, 0u));
  if (meet.y != 0u) {
    if (meet.x < low) {
      z = meet.x;
      at = line_handed(line, z);
    } else if (meet.x >= up.y) {
      z = up.y;
      at = parent_handed;
    } else if (meet.x >= up.x) {
      z = up.x;
      at = grand_handed;
    } else {
      z = low;
      at = low_handed;
    }
  }
  for (var i = 0u; i < 33u && z < zoom; i += 1u) {
    z += 1u;
    at = world_child(at, z, over(n, zoom, z));
  }
  let a = world_node(at, zoom, n, zoom, zoom, eye_cut(CARVE), eye_cut(HIDE)).asset.x;
  return fills(a) && ((a >> FILL_SHIFT) & (EIGHTHS - 1u)) == 0u;
}

/// One cell along `axis` by `s`.
fn unit(axis: u32, s: i32) -> vec3<i32> {
  return vec3<i32>(select(0i, s, axis == 0u), select(0i, s, axis == 1u), select(0i, s, axis == 2u));
}

/// Edge `k`'s way across, along `u` or `v` (`face::edge`).
fn edge(u: u32, v: u32, k: u32) -> vec3<i32> {
  return unit(select(v, u, k < 2u), 1i - 2i * i32(k & 1u));
}

/// The bits of face `face` of the hit `c` at `zoom` (`face::word`), each cell asked off the
/// line down to it (`hides`): bit `i` answers the `i`th cell asked, and a cell is not asked
/// where a bit already set decides it.
fn face_word(
  face: u32,
  line: u32,
  low: u32,
  low_handed: vec4<u32>,
  up: vec2<u32>,
  grand_handed: vec4<u32>,
  parent_handed: vec4<u32>,
  c: vec3<i32>,
  zoom: u32,
) -> u32 {
  let axis = face / 2u;
  let n = unit(axis, 1i - 2i * i32(face & 1u));
  let u = (axis + 1u) % 3u;
  let v = (axis + 2u) % 3u;
  var w = 0u;
  for (var i = 0u; i < FACE_WORD_BITS; i += 1u) {
    let field = i / FACE_EDGES;
    let k = i % FACE_EDGES;
    let a = k & 1u;
    let b = 2u + (k >> 1u);
    // The cell asked, and whether what is set already leaves it open.
    var d = edge(u, v, k) + n;
    var open = true;
    if (field == 1u) {
      d = edge(u, v, k);
      open = ((w >> (FACE_RISE_SHIFT + k)) & 1u) == 0u;
    } else if (field > 1u) {
      d = edge(u, v, a) + edge(u, v, b) + n;
      open = ((w >> FACE_RISE_SHIFT) & ((1u << a) | (1u << b))) == 0u;
    }
    // A line where the cell beside does not hide; a rise or a corner where the one in front
    // does (`face::beside`).
    if (open) {
      let beside = vec3<i32>(wrap_idx(zoom, c.x + d.x), c.y + d.y, c.z + d.z);
      if (hides(line, low, low_handed, up, grand_handed, parent_handed, c, zoom, beside) != (field == 1u)) {
        w |= 1u << i;
      }
    }
  }
  return w;
}

@compute @workgroup_size(RASTER_FACES_LANES)
fn face_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  // The asks of the holes' stops hold a place for each stop, of the samples one for
  // each sample (`beam::Grid::stop_asks_at`, `asks_at`).
  let asks = args[RASTER_ARGS_ASKS];
  let samples = eye_word(GRID + GRID_SAMPLES) * eye_word(GRID + GRID_SAMPLES + 1u);
  let room = select(samples, HITS, asks == eye_word(GRID + GRID_STOP_ASKS_AT));
  if (id.x >= in_room(args[RASTER_ARGS_INSTANCES], room)) {
    return;
  }
  let a = asks + id.x * ASK_WORDS;
  let to = beams[a + ASK_TO];
  if (to == ASK_NONE) {
    return;
  }
  // A drawable entry's ask names its entry, the node the world names (pxl-engine
  // `record::TO_RECORD`, `node_cache::W_NODE`), a hit's its node among the beam rays' words.
  var hit: vec4<i32>;
  if ((to & RECORD_TO) != 0u) {
    let n = beams[a + ASK_NODE] + W_NODE;
    hit = vec4<i32>(i32(node_cache[n + 1u]), i32(node_cache[n + 2u]), i32(node_cache[n + 3u]), i32(node_cache[n]));
  } else {
    hit = node_idx(beams[a + ASK_NODE]);
  }
  let c = hit.xyz;
  let zoom = u32(hit.w);
  let line = beams[a + ASK_LINE];
  let held = node_idx(line + LINE_NODE);
  // Where the hit's own descent starts: the deepest node of the line over it, or its root
  // where none is (`support::sub_line`).
  let meet = shared_zoom(held.xyz, u32(held.w), c, zoom);
  var low = 0u;
  var low_handed = world_root(0u, over(c, zoom, 0u));
  if (meet.y != 0u) {
    low = meet.x;
    low_handed = line_handed(line, low);
  }
  let d = descent(c, zoom, low, low_handed);
  let word = face_word(beams[a + ASK_FACE], line, low, low_handed, d.up, d.grand_handed, d.parent_handed, c, zoom);
  // A drawable entry's face's word into the node cache (pxl-engine `record::TO_RECORD`), a
  // hit's beside it.
  if ((to & RECORD_TO) != 0u) {
    node_cache[to & ~RECORD_TO] = word;
  } else {
    beams[to] = word;
  }
}
