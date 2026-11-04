// pxl-render/forget.wgsl

// What the observed pyramid kept forgotten, a thread a slot, where what the world's
// query reads changed (pxl-engine pyramid.rs `FORGETS`): every slot's key, or the key of
// each slot over a brick that moved (`forgets`), over a box of what is placed or cut that
// changed (`over_box`), or handed a slot of a child group of what is placed that was
// written (`reads_written`).

@group(0) @binding(9) var<storage, read_write> pyramid: array<u32>;

/// A clip level's moved words, its entries a bit each, a row of `ACROSS` bits after
/// another (`pyramid::LEVEL_WORDS`).
const LEVEL_WORDS: u32 = CLIP_ENTRIES / 32u;
const ROWS_A_WORD: u32 = 32u / ACROSS;

/// The entries of a clip level that hold the bricks from `q0` to `q1`, X then Y, a bit
/// each as its moved words lay them: every one where they span the level's side either
/// way, as the clip level wraps (`pyramid::entries_of`).
fn entries_of(q0: vec2<i32>, q1: vec2<i32>) -> array<u32, LEVEL_WORDS> {
  var mask: array<u32, LEVEL_WORDS>;
  let n = q1 - q0 + vec2<i32>(1);
  if (n.x >= i32(ACROSS) || n.y >= i32(ACROSS)) {
    for (var k = 0u; k < LEVEL_WORDS; k += 1u) {
      mask[k] = 0xffffffffu;
    }
    return mask;
  }
  let m = i32(ACROSS) - 1i;
  // The row's columns, from the first round the clip level.
  let run = (1u << u32(n.x)) - 1u;
  let s = u32(q0.x & m);
  let row = ((run << s) | (run >> (ACROSS - s))) & (0xffffffffu >> (32u - ACROSS));
  for (var j = 0i; j < n.y; j += 1i) {
    let r = u32((q0.y + j) & m);
    mask[r / ROWS_A_WORD] |= row << (ACROSS * (r % ROWS_A_WORD));
  }
  return mask;
}

/// Whether the node of `zoom` at column `c` reads a brick that moved, the moved
/// entries' words at `MOVED_AT` (`pyramid::forgets`).
fn forgets(zoom: u32, c: vec2<i32>) -> bool {
  for (var w = 0u; w < EYE_CLIP_LEVELS; w += 1u) {
    let cell = FIRST_CELL + w;
    // The node's cells at the clip level's zoom, first and last, then its bricks.
    var lo: vec2<i32>;
    var hi: vec2<i32>;
    if (zoom <= cell) {
      let s = cell - zoom;
      lo = c << vec2<u32>(s);
      hi = ((c + vec2<i32>(1)) << vec2<u32>(s)) - vec2<i32>(1);
    } else {
      let s = zoom - cell;
      lo = c >> vec2<u32>(s);
      hi = lo;
    }
    let mask = entries_of(lo >> vec2<u32>(BRICK_SHIFT), hi >> vec2<u32>(BRICK_SHIFT));
    let at = PYRAMID_MOVED_AT + w * LEVEL_WORDS;
    for (var k = 0u; k < LEVEL_WORDS; k += 1u) {
      if ((pyramid[at + k] & mask[k]) != 0u) {
        return true;
      }
    }
  }
  return false;
}

/// Whether the node of `zoom` at column `c` stands over the box `b` of leaf cells
/// `[x0, y0, x1, y1)`, X round the world (`pyramid::over_box`, `overwrite::cut_box`).
fn over_box(zoom: u32, c: vec2<i32>, b: vec4<i32>) -> bool {
  var n: vec4<i32>;
  if (zoom <= LEAF_CELL) {
    let s = LEAF_CELL - zoom;
    n = vec4<i32>(c << vec2<u32>(s), (c + vec2<i32>(1)) << vec2<u32>(s));
  } else {
    let s = zoom - LEAF_CELL;
    let at = c >> vec2<u32>(s);
    n = vec4<i32>(at, at + vec2<i32>(1));
  }
  let world = 1i << LEAF_CELL;
  let nw = n.z - n.x;
  let bw = b.z - b.x;
  // The box's first column from the node's, the short way round.
  let from_node = wrap_idx(LEAF_CELL, b.x - n.x);
  let across = nw >= world
    || bw >= world
    || (from_node < nw && from_node + bw > 0i)
    || from_node + bw > world
    || from_node + world < nw;
  return across && b.y < n.w && b.w > n.y;
}

/// Whether what was handed a node of what is placed, its slot word `placed`, answers
/// from a slot of the child group written whose first slot is `first`
/// (`pyramid::reads_written`).
fn reads_written(placed: u32, first: u32) -> bool {
  return placed != SLOT_NOTHING && (placed >> SLOT_SHIFT) - first < CHILDREN;
}

/// The key of a split a moved brick lies under, forgotten: it holds at no zooms asked,
/// and is no empty slot (`pyramid::forgotten_split`).
fn forgotten_split(key: u32) -> u32 {
  let zooms = (1u << ZOOM_BITS) - 1u;
  return (key & ((1u << TAG_BITS) - 1u)) | (zooms << TAG_BITS) | (zooms << (TAG_BITS + ZOOM_BITS));
}

@compute @workgroup_size(ROW_LANES, 1, 1)
fn forget_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x >= ENTRIES) {
    return;
  }
  let w = ENTRIES_AT + id.x * ENTRY_WORDS;
  if (pyramid[PYRAMID_FORGETS] == FORGETS_ALL) {
    pyramid[w + W_KEY] = 0u;
    return;
  }
  let key = pyramid[w + W_KEY];
  if (key == 0u) {
    return;
  }
  let zoom = pyramid[w + W_NODE];
  let c = vec2<i32>(i32(pyramid[w + W_NODE + 1u]), i32(pyramid[w + W_NODE + 2u]));
  if (forgets(zoom, c)) {
    let split = (pyramid[w + W_KIND] & KIND_MASK) == KIND_SPLIT;
    pyramid[w + W_KEY] = select(0u, forgotten_split(key), split);
    return;
  }
  // Or handed a slot of a child group of what is placed that was written.
  let placed = pyramid[w + W_PLACED];
  for (var f = 0u; f < pyramid[PYRAMID_WRITTEN_AT]; f += 1u) {
    if (reads_written(placed, pyramid[PYRAMID_WRITTEN_AT + 1u + f])) {
      pyramid[w + W_KEY] = 0u;
      return;
    }
  }
  // Or over a box of what is placed or cut that changed.
  for (var b = 0u; b < pyramid[PYRAMID_BOXES_AT]; b += 1u) {
    let at = PYRAMID_BOXES_AT + 1u + 4u * b;
    let changed = vec4<i32>(
      i32(pyramid[at]),
      i32(pyramid[at + 1u]),
      i32(pyramid[at + 2u]),
      i32(pyramid[at + 3u]),
    );
    if (over_box(zoom, c, changed)) {
      pyramid[w + W_KEY] = 0u;
      return;
    }
  }
}
