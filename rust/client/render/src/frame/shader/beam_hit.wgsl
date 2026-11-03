// pxl-render/beam_hit.wgsl

// The beam rays' walk over the observed pyramid alone (walk.wgsl), its line kept as
// it goes, in two rounds. What the first leaves it lists, with the node it stopped
// at, once a node, for `ask_cs` to ask of the world; the second walks what it listed
// again. What that leaves `beam_cs` walks, writing what it asks; one that ends is
// left as that leaves it (pxl-shader observer.rs `beam`, beam.rs `Leaving`).

/// The next's dispatch, then how many are listed: only the lists' own atomics.
@group(0) @binding(10) var<storage, read_write> left: array<atomic<u32>, 4>;
/// `ask_cs`'s dispatch and how many, a table that tells a node seen, then the nodes
/// (`ASK_WORDS` each), past the room none.
@group(0) @binding(11) var<storage, read_write> asks: array<atomic<u32>>;

/// The node `w` stopped at listed for `ask_cs`, once, as far as the table tells: two
/// nodes a slot apart ask twice, which costs one ask and answers the same.
fn ask(w: Walked) {
  // Unvisited, it asked nothing.
  if (w.asked.y == 0u && w.asked.x == 0u) {
    return;
  }
  let c = idx_of(w.corner, w.zoom, w.roots, vec3<i32>(i32(eye_word(ORIGIN)), i32(eye_word(ORIGIN + 1u)), i32(eye_word(ORIGIN + 2u))) >> vec3<u32>(31u));
  let h = bucket_of(w.zoom, c) ^ (w.zoom * 0x9E3779B1u) ^ (w.asked.y << 7u);
  let seen = h | 1u;
  if (atomicExchange(&asks[ASKS_TABLE + (h >> 3u) % ASKS_SLOTS], seen) == seen) {
    return;
  }
  let place = atomicAdd(&asks[3], 1u);
  if (place >= ASKS_ROOM) {
    return;
  }
  if (place % 64u == 0u) {
    atomicAdd(&asks[0], 1u);
  }
  let at = ASKS_LIST + place * ASK_NODE_WORDS;
  atomicStore(&asks[at], w.zoom);
  atomicStore(&asks[at + 1u], u32(c.x));
  atomicStore(&asks[at + 2u], u32(c.y));
  atomicStore(&asks[at + 3u], u32(c.z));
  atomicStore(&asks[at + 4u], w.asked.x);
  atomicStore(&asks[at + 5u], w.asked.y);
}

/// Corner `id`'s ray; whether it ended, its words left. What it did not end the
/// caller lists at `list`.
fn one(id: vec2<u32>, list: u32) -> Walked {
  let corners = vec2<u32>(eye_word(GRID + 7u), eye_word(GRID + 8u));
  let past = f32(eye_word(GRID + 4u));
  let s = eye_f32(GRID);
  let tx = -eye_f32(TAN) + (f32(id.x) - past) * s;
  let ty = eye_f32(TAN + 1u) - (f32(id.y) - past) * s;
  let idx = id.y * corners.x + id.x;
  let base = idx * LINE_WORDS;
  let w = walk(vec2<f32>(tx, ty), vec2<u32>(0u), base);
  if (w.ends == 0u || w.ends == 4u) {
    let place = atomicAdd(&left[3], 1u);
    if (place % 64u == 0u) {
      atomicAdd(&left[0], 1u);
    }
    beams[list + 1u + place] = id.x | (id.y << 16u);
    return w;
  }
  let at = base + LINE_NODE;
  beams[at + NODE_ZOOM] = w.zoom;
  beams[at + NODE_CORNER] = w.corner.x;
  beams[at + NODE_CORNER + 1u] = w.corner.y;
  beams[at + NODE_CORNER + 2u] = w.corner.z;
  beams[at + NODE_ROOTS] = u32(w.roots.x);
  beams[at + NODE_ROOTS + 1u] = u32(w.roots.y);
  beams[at + NODE_ROOTS + 2u] = u32(w.roots.z);
  beams[base + LINE_VISITS] = w.visits;
  // How far the rays about it start (`Grid::hand_at`, `Grid::hand_dist`).
  beams[corners.x * corners.y * LINE_WORDS + idx] = bitcast<u32>(w.t * eye_f32(GRID + 1u));
  return w;
}

@compute @workgroup_size(8, 8, 1)
fn beam_hit_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let corners = vec2<u32>(eye_word(GRID + 7u), eye_word(GRID + 8u));
  if (id.x >= corners.x || id.y >= corners.y) {
    return;
  }
  let w = one(id.xy, eye_word(GRID + 19u));
  if (w.ends == 0u || w.ends == 4u) {
    ask(w);
  }
}

/// What the first round left, once the world was asked: a thread's place among them,
/// workgroups in a row (pxl-shader eye.rs `left_at`).
@compute @workgroup_size(8, 8, 1)
fn beam_again_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let corners = vec2<u32>(eye_word(GRID + 7u), eye_word(GRID + 8u));
  let first = eye_word(GRID + 19u);
  let place = (id.x / 8u) * 64u + (id.y % 8u) * 8u + id.x % 8u;
  if (place >= beams[first]) {
    return;
  }
  let s = beams[first + 1u + place];
  one(vec2<u32>(s & 0xffffu, s >> 16u), first + 1u + corners.x * corners.y);
}
