// pxl-render/forget.wgsl

// Forgets what the node cache keeps where what the world's query reads has changed, one
// thread for each slot (pxl-engine node_cache.rs `FORGETS`). A frame forgets every slot's key,
// or only the key of each slot whose node reads a brick that moved (`forgets`), stands over a
// box of what is placed or cut that changed (`over_rect`), or was handed a slot of a child
// group of what is placed that was written (`reads_written`). A slot that draws (pxl-engine
// `record`) and stays kept loses its faces and their words where a node beside it is
// forgotten by those rules (`changed_beside`). A face's word is asked off the cells beside the
// face, so the change beside may have made it wrong.

@group(0) @binding(9) var<storage, read_write> node_cache: array<u32>;

/// The words of one clip level's moved bits, which hold one bit for each of its entries, in
/// rows of `ACROSS` bits (pxl-engine `node_cache::CHANGED_AT`).
const LEVEL_WORDS: u32 = CLIP_ENTRIES / 32u;
const ROWS_A_WORD: u32 = 32u / ACROSS;

/// Word `k` of the bits of the clip level's entries that hold the bricks from `q0` to `q1`,
/// laid out as the moved bits are. The clip level wraps, so where the bricks span its side
/// either way, every entry holds one of them.
fn entries_word(q0: vec2<i32>, q1: vec2<i32>, k: u32) -> u32 {
  let n = q1 - q0 + vec2<i32>(1);
  if (n.x >= i32(ACROSS) || n.y >= i32(ACROSS)) {
    return 0xffffffffu;
  }
  let m = i32(ACROSS) - 1i;
  // The row's bits for the bricks' columns, from the first column round the clip level.
  let run = (1u << u32(n.x)) - 1u;
  let s = u32(q0.x & m);
  let row = ((run << s) | (run >> (ACROSS - s))) & (0xffffffffu >> (32u - ACROSS));
  var word = 0u;
  for (var j = 0i; j < n.y; j += 1i) {
    let r = u32((q0.y + j) & m);
    if (r / ROWS_A_WORD == k) {
      word |= row << (ACROSS * (r % ROWS_A_WORD));
    }
  }
  return word;
}

/// Whether the node of `zoom` at column `c`, whose answer reads the clip levels up to
/// `finest`, reads a brick that moved. A query reads no clip level finer than it asked
/// (clip.wgsl `step_to`, `clip_zoom`). A node no wider than a brick's levels reads only the
/// bricks held present (`step_to`). It reads a moved brick where one lies in the box of the
/// clip level's moved bricks that are held (`CHANGED_BRICKS_AT`), and its entry's bit is set
/// among the moved bits at `CHANGED_AT`. A wider node reads the clip level's fold (`level_bounds`),
/// which bounds every brick wanted, held or not, so any moved entry under it counts. A node
/// coarser than `MIN_ZOOM` reads no brick: its ground is a split whatever the clip levels hold
/// (world_split.wgsl `ground_split`, world.wgsl `ground_cell`).
fn forgets(zoom: u32, finest: u32, c: vec2<i32>) -> bool {
  if (zoom < MIN_ZOOM) {
    return false;
  }
  for (var w = 0u; w < EYE_CLIP_LEVELS && FIRST_CELL_ZOOM + w <= max(finest, FIRST_CELL_ZOOM); w += 1u) {
    let cell_zoom = FIRST_CELL_ZOOM + w;
    // The first and last of the node's cells at the clip level's zoom, then the bricks that
    // hold them.
    var lo: vec2<i32>;
    var hi: vec2<i32>;
    if (zoom <= cell_zoom) {
      let s = cell_zoom - zoom;
      lo = c << vec2<u32>(s);
      hi = ((c + vec2<i32>(1)) << vec2<u32>(s)) - vec2<i32>(1);
    } else {
      let s = zoom - cell_zoom;
      lo = c >> vec2<u32>(s);
      hi = lo;
    }
    let q0 = lo >> vec2<u32>(BRICK_SHIFT);
    let q1 = hi >> vec2<u32>(BRICK_SHIFT);
    let b = NODE_CACHE_CHANGED_BRICKS_AT + 4u * w;
    let first = vec2<i32>(i32(node_cache[b]), i32(node_cache[b + 1u]));
    let last = vec2<i32>(i32(node_cache[b + 2u]), i32(node_cache[b + 3u]));
    if (cell_zoom <= zoom + BRICK_MIPS && (any(q1 < first) || any(q0 > last))) {
      continue;
    }
    let at = NODE_CACHE_CHANGED_AT + w * LEVEL_WORDS;
    for (var k = 0u; k < LEVEL_WORDS; k += 1u) {
      if ((node_cache[at + k] & entries_word(q0, q1, k)) != 0u) {
        return true;
      }
    }
  }
  return false;
}

/// Whether the node of `zoom` at column `c` stands over the box `b` of leaf cells,
/// `[x0, y0, x1, y1)`, with X taken round the world (`object::cut_rect`).
fn over_rect(zoom: u32, c: vec2<i32>, b: vec4<i32>) -> bool {
  var n: vec4<i32>;
  if (zoom <= LEAF_CELL_ZOOM) {
    let s = LEAF_CELL_ZOOM - zoom;
    n = vec4<i32>(c << vec2<u32>(s), (c + vec2<i32>(1)) << vec2<u32>(s));
  } else {
    let s = zoom - LEAF_CELL_ZOOM;
    let at = c >> vec2<u32>(s);
    n = vec4<i32>(at, at + vec2<i32>(1));
  }
  let world = 1i << LEAF_CELL_ZOOM;
  let nw = n.z - n.x;
  let bw = b.z - b.x;
  // How far the box's first column lies from the node's, the short way round.
  let from_node = wrap_idx(LEAF_CELL_ZOOM, b.x - n.x);
  let across = nw >= world
    || bw >= world
    || (from_node < nw && from_node + bw > 0i)
    || from_node + bw > world
    || from_node + world < nw;
  return across && b.y < n.w && b.w > n.y;
}

/// Whether a node was handed its part of what is placed from a slot of the written child group
/// whose first slot is `first`. `placed` is the slot word the node was handed.
fn reads_written(placed: u32, first: u32) -> bool {
  return placed != SLOT_NOTHING && (placed >> SLOT_SHIFT) - first < CHILDREN;
}

/// The forgotten key of a split that a moved brick lies under. It keeps the node at no zooms a
/// walk asks, so the walk that asks the world asks the node again. It is not an empty slot, so
/// the walk over the node cache still goes into the split (cached.wgsl) and takes none of what
/// the split read of the brick. Every moved brick lies under the root and the chain of splits
/// over it, and if their slots were emptied, every ray of the frame would stop at them.
fn forgotten_split(key: u32) -> u32 {
  let zooms = (1u << ZOOM_BITS) - 1u;
  return (key & ((1u << TAG_BITS) - 1u)) | (zooms << TAG_BITS) | (zooms << (TAG_BITS + ZOOM_BITS));
}

@compute @workgroup_size(ROW_LANES, 1, 1)
fn forget_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x >= ENTRIES) {
    return;
  }
  let key_idx = key_at(id.x);
  if (node_cache[NODE_CACHE_FORGETS] == FORGETS_ALL) {
    node_cache[key_idx] = 0u;
    return;
  }
  let key = node_cache[key_idx];
  if (key == 0u) {
    return;
  }
  let w = entry_at(id.x);
  let zoom = node_cache[w + W_NODE];
  let c = vec2<i32>(i32(node_cache[w + W_NODE + 1u]), i32(node_cache[w + W_NODE + 2u]));
  // `finest` is the key's near zoom. A cell kept by its node alone holds only its whole bit in
  // that field, so its own zoom is taken (`node_cache::key_zooms`).
  if (forgets(zoom, max(zoom, key >> (TAG_BITS + ZOOM_BITS)), c)) {
    let split = (node_cache[w + W_KIND] & KIND_MASK) == KIND_SPLIT;
    node_cache[key_idx] = select(0u, forgotten_split(key), split);
    return;
  }
  // The slot is forgotten where its node was handed a slot of a written child group of what is
  // placed. The head lists at most `NODE_CACHE_WRITTEN_CAP` written child groups and
  // `NODE_CACHE_RECTS` boxes (pxl-engine node_cache.rs).
  let placed = node_cache[w + W_PLACED];
  let written = min(node_cache[NODE_CACHE_WRITTEN_AT], NODE_CACHE_WRITTEN_CAP);
  for (var f = 0u; f < written; f += 1u) {
    if (reads_written(placed, node_cache[NODE_CACHE_WRITTEN_AT + 1u + f])) {
      node_cache[key_idx] = 0u;
      return;
    }
  }
  // The slot is forgotten where its node stands over a box of what is placed or cut that
  // changed.
  let rects = min(node_cache[NODE_CACHE_RECTS_AT], NODE_CACHE_RECTS);
  for (var b = 0u; b < rects; b += 1u) {
    let at = NODE_CACHE_RECTS_AT + 1u + 4u * b;
    let changed = vec4<i32>(
      i32(node_cache[at]),
      i32(node_cache[at + 1u]),
      i32(node_cache[at + 2u]),
      i32(node_cache[at + 3u]),
    );
    if (over_rect(zoom, c, changed)) {
      node_cache[key_idx] = 0u;
      return;
    }
  }
  let state = node_cache[state_at(id.x)];
  if ((state & (RECORD_FACES | RECORD_ASKED)) != 0u && changed_beside(zoom, c)) {
    node_cache[state_at(id.x)] = state & ~(RECORD_FACES | RECORD_ASKED);
    for (var face = 0u; face < 6u; face += 1u) {
      node_cache[face_at(id.x, face)] = 0u;
    }
  }
}

/// Whether a node of `zoom` beside the one at column `c`, across a face or an edge, is
/// forgotten because it reads a brick that moved or stands over a box that changed (`forgets`,
/// `over_rect`).
fn changed_beside(zoom: u32, c: vec2<i32>) -> bool {
  let rects = min(node_cache[NODE_CACHE_RECTS_AT], NODE_CACHE_RECTS);
  for (var dy = -1i; dy <= 1i; dy += 1i) {
    for (var dx = -1i; dx <= 1i; dx += 1i) {
      let at = vec2<i32>(wrap_idx(zoom, c.x + dx), c.y + dy);
      if (forgets(zoom, zoom, at)) {
        return true;
      }
      for (var b = 0u; b < rects; b += 1u) {
        let w = NODE_CACHE_RECTS_AT + 1u + 4u * b;
        let changed = vec4<i32>(
          i32(node_cache[w]),
          i32(node_cache[w + 1u]),
          i32(node_cache[w + 2u]),
          i32(node_cache[w + 3u]),
        );
        if (over_rect(zoom, at, changed)) {
          return true;
        }
      }
    }
  }
  return false;
}
