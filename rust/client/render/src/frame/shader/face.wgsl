// pxl-render/face.wgsl

// Asks the world (world.wgsl) what lies around the face each hit's ray entered, and writes the
// face's word (pxl-engine `face::word`). There is one ask for each hit, whether the hit is a
// drawable entry's voxel or the stop of a missed pixel, and the args the pass is bound to say
// which list of asks it runs. Each cell beside the hit is asked from the deepest node it
// shares with the line down to the hit (`support::asset`). That line follows the beam ray's
// line that the hit's cell resumed from, as the beam ray left it, down to the deepest node the
// two share. Below that node, the hit's own descent hands each node down from its parent
// (`support::sub_line`).

@group(0) @binding(8) var<storage, read_write> beams: array<u32>;
/// The node cache. An ask may name a drawable entry's face word as the word it writes
/// (pxl-engine `record::TO_RECORD`, `node_cache::R_FACES`). Each face has one ask, so each word
/// has one writer.
@group(0) @binding(9) var<storage, read_write> node_cache: array<u32>;
/// The raster's args, which say where the pass's asks lie (`raster::ARGS_ASKS`) and how many
/// there are (`raster::ARGS_INSTANCES`).
@group(0) @binding(16) var<storage, read> args: array<u32>;

/// The node at zoom `z` over node `c` of `zoom`, where `z` is no finer than `zoom`
/// (`object::ancestor_at`).
fn over(c: vec3<i32>, zoom: u32, z: u32) -> vec3<i32> {
  let steps = zoom - z;
  let p = path(zoom, c);
  return vec3<i32>(c.x >> steps, (p.y >> steps) - y_start(z), (p.z >> steps) - z_start(z));
}

/// The deepest zoom, no finer than either node, at which node `a` of `za` and node `c` of
/// `zoom` lie under one node. The second word is 1 where they share a node at all, and 0 where
/// they lie under two roots (`support::shared_zoom`).
fn shared_zoom(a: vec3<i32>, za: u32, c: vec3<i32>, zoom: u32) -> vec2<u32> {
  var z = min(zoom, za);
  for (var i = 0u; i < 33u && z > 0u && any(over(a, za, z) != over(c, zoom, z)); i += 1u) {
    z -= 1u;
  }
  return vec2<u32>(z, select(0u, 1u, all(over(a, za, z) == over(c, zoom, z))));
}

/// What was handed to the node at zoom `z` on the beam ray's line that starts at word `line`
/// (pxl-engine `beam::LINE_INHERITANCE`, `beam::LINE_STEP`).
fn line_inheritance(line: u32, z: u32) -> vec4<u32> {
  let k = line + LINE_INHERITANCE + LINE_STEP * z;
  return vec4<u32>(beams[k], beams[k + 1u], beams[k + 2u], beams[k + 3u]);
}

/// The node held at word `k`, laid out as a line and a sample each hold one
/// (`beam::NODE_WORDS`). It is returned as its index (`Node::idx`), its roots taken from the
/// eye's origin, with its zoom in `w`.
fn node_idx(k: u32) -> vec4<i32> {
  let o = vec3<i32>(i32(eye_word(ORIGIN)), i32(eye_word(ORIGIN + 1u)), i32(eye_word(ORIGIN + 2u)));
  let r0 = vec3<i32>(o.x >> 31u, 0i, i32(u32(o.z) + Z_START) >> 31u);
  let zoom = beams[k + NODE_ZOOM];
  let corner = vec3<u32>(beams[k + NODE_CORNER], beams[k + NODE_CORNER + 1u], beams[k + NODE_CORNER + 2u]);
  let roots = vec3<i32>(i32(beams[k + NODE_ROOTS]), i32(beams[k + NODE_ROOTS + 1u]), i32(beams[k + NODE_ROOTS + 2u]));
  return vec4<i32>(idx_of(corner, zoom, roots, r0), i32(zoom));
}

/// The hit's own descent from where it leaves the line (`support::sub_line`). It keeps what was
/// handed to the hit's grandparent and parent, whose zooms are in `up`. Where the descent starts
/// no higher than one of them, the descent's first node stands in for it (`descent`).
struct Descent {
  up: vec2<u32>,
  grand_inheritance: vec4<u32>,
  parent_inheritance: vec4<u32>,
}

/// The descent of the hit `c` at `zoom` from zoom `low`, where it was handed
/// `low_inheritance`, each node under that handed down by its parent. It is worked out once for
/// the hit, and every cell beside the hit reads it (`hides`).
fn descent(c: vec3<i32>, zoom: u32, low: u32, low_inheritance: vec4<u32>) -> Descent {
  var up = vec2<u32>(low);
  var grand = low_inheritance;
  var parent = low_inheritance;
  var at = low_inheritance;
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

/// Whether the cell `n` at `zoom` beside the hit `c` hides its cube whole (`answer::whole`), as
/// the world answers it at that zoom. The cell is asked off the line down to the hit. Above
/// `low`, that line is the beam ray's line at word `line`, and from `low` down it is the hit's
/// own descent (`descent`; `support::sub_line`, `JoinedLine`). The descent was handed
/// `low_inheritance` at `low`, and `grand_inheritance` and `parent_inheritance` at the zooms in
/// `up`. The ask starts from the deepest node of that line that is over the cell and held here,
/// and each node under it is handed down by its parent. Where no node of the line is over the
/// cell, the ask starts from the cell's own root (`support::asset`).
fn hides(
  line: u32,
  low: u32,
  low_inheritance: vec4<u32>,
  up: vec2<u32>,
  grand_inheritance: vec4<u32>,
  parent_inheritance: vec4<u32>,
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
      at = line_inheritance(line, z);
    } else if (meet.x >= up.y) {
      z = up.y;
      at = parent_inheritance;
    } else if (meet.x >= up.x) {
      z = up.x;
      at = grand_inheritance;
    } else {
      z = low;
      at = low_inheritance;
    }
  }
  for (var i = 0u; i < 33u && z < zoom; i += 1u) {
    z += 1u;
    at = world_child(at, z, over(n, zoom, z));
  }
  let a = world_node(at, zoom, n, zoom, zoom, eye_cut(CARVE), eye_cut(HIDE)).asset.x;
  return fills(a) && ((a >> FILL_SHIFT) & (EIGHTHS - 1u)) == 0u;
}

fn unit(axis: u32, s: i32) -> vec3<i32> {
  return vec3<i32>(select(0i, s, axis == 0u), select(0i, s, axis == 1u), select(0i, s, axis == 2u));
}

/// The direction that crosses edge `k` of a face whose axes are `u` and `v` (`face::edge`).
fn edge(u: u32, v: u32, k: u32) -> vec3<i32> {
  return unit(select(v, u, k < 2u), 1i - 2i * i32(k & 1u));
}

/// The word of face `face` of the hit `c` at `zoom` (`face::word`). Each cell beside the hit is
/// asked off the line down to it (`hides`). Bit `i` answers the `i`th cell asked, and a cell is
/// not asked where a bit already set decides it.
fn face_word(
  face: u32,
  line: u32,
  low: u32,
  low_inheritance: vec4<u32>,
  up: vec2<u32>,
  grand_inheritance: vec4<u32>,
  parent_inheritance: vec4<u32>,
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
    // The cell to ask, and whether the bits set so far leave it to be asked.
    var d = edge(u, v, k) + n;
    var open = true;
    if (field == 1u) {
      d = edge(u, v, k);
      open = ((w >> (FACE_RISE_SHIFT + k)) & 1u) == 0u;
    } else if (field > 1u) {
      d = edge(u, v, a) + edge(u, v, b) + n;
      open = ((w >> FACE_RISE_SHIFT) & ((1u << a) | (1u << b))) == 0u;
    }
    // A line bit is set where the cell beside does not hide, and a rise or a corner bit where
    // the cell in front does (`face::beside`).
    if (open) {
      let beside = vec3<i32>(wrap_idx(zoom, c.x + d.x), c.y + d.y, c.z + d.z);
      if (hides(line, low, low_inheritance, up, grand_inheritance, parent_inheritance, c, zoom, beside) != (field == 1u)) {
        w |= 1u << i;
      }
    }
  }
  return w;
}

@compute @workgroup_size(RASTER_FACES_LANES)
fn face_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  // The holes' stops have room for `HITS_A_FRAME` asks, and the samples for one ask a sample
  // (`beam::Grid::hit_asks_at`, `asks_at`). Where the asks start tells which list this is.
  let asks = args[RASTER_ARGS_ASKS];
  let samples = eye_word(GRID + GRID_SAMPLES) * eye_word(GRID + GRID_SAMPLES + 1u);
  let cap = select(samples, HITS_A_FRAME, asks == eye_word(GRID + GRID_HIT_ASKS_AT));
  if (id.x >= in_cap(args[RASTER_ARGS_INSTANCES], cap)) {
    return;
  }
  let a = asks + id.x * ASK_WORDS;
  let to = beams[a + ASK_TO];
  if (to == ASK_NONE) {
    return;
  }
  // A drawable entry's ask names its entry, which holds its node as the world names it
  // (pxl-engine `record::TO_RECORD`, `node_cache::W_NODE`). A hit's ask names its node among
  // the beam rays' words.
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
  // The hit's own descent starts at the deepest node of the line over it, or at its root where
  // no node of the line is over it (`support::sub_line`).
  let meet = shared_zoom(held.xyz, u32(held.w), c, zoom);
  var low = 0u;
  var low_inheritance = world_root(0u, over(c, zoom, 0u));
  if (meet.y != 0u) {
    low = meet.x;
    low_inheritance = line_inheritance(line, low);
  }
  let d = descent(c, zoom, low, low_inheritance);
  let word = face_word(beams[a + ASK_FACE], line, low, low_inheritance, d.up, d.grand_inheritance, d.parent_inheritance, c, zoom);
  // A drawable entry's face word goes into the node cache (pxl-engine `record::TO_RECORD`). A
  // hit's goes to the word its ask names among the beam rays' words.
  if ((to & RECORD_TO) != 0u) {
    node_cache[to & ~RECORD_TO] = word;
  } else {
    beams[to] = word;
  }
}
