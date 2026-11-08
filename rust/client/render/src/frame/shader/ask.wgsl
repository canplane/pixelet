// pxl-render/ask.wgsl

// How the walk that may ask the world answers a node (walk.wgsl): from the node cache, or by
// asking the world there and then and keeping the answer (pxl-engine `cast::walk`,
// `node_cache`). There is no stack. A node asked of the world finds what it inherits from its
// parent's words where the walk holds them, and otherwise from the top (`world_root`). Every
// word of the node cache is atomic, as a walk claims a slot by exchanging its key. No walk
// uses compare-and-swap: Safari's WGSL compiler writes Metal for `atomicCompareExchangeWeak`
// that Metal's compiler refuses. The pass that includes this part names the tag its walks keep
// under (`WALK_PASS`), where its asks are counted (`asked_at`), and whether it may ask another
// (`may_ask`).

@group(0) @binding(9) var<storage, read_write> node_cache: array<atomic<u32>>;

fn node_cache_word(k: u32) -> u32 {
  return atomicLoad(&node_cache[k]);
}

/// Word `k` as it stands now. It is read by an operation that writes, as a plain atomic load
/// may return a value some while old.
fn node_cache_now(k: u32) -> u32 {
  return atomicOr(&node_cache[k], 0u);
}

/// Whether a node may be asked of the world, which it may while one more ask stays within the
/// frame's budget (`ray::ASKS_A_FRAME`). The ask is counted as it is taken.
fn may_ask() -> bool {
  return atomicAdd(&node_cache[ASKED], 1u) < ASKS_A_FRAME;
}

/// This pass's tag: the frame's number times `PASSES`, plus the pass. A walk reads it once.
fn walk_tag() -> u32 {
  return PASSES * node_cache_word(FRAME) + WALK_PASS;
}

/// Whether slot `slot`'s entry is the node. The zoom is compared first, and the corner's three
/// words are loaded and compared together only when it matches: an entry of another node mostly
/// differs in its zoom already, and every word is an atomic load.
fn is_node(slot: u32, zoom: u32, c: vec3<i32>) -> bool {
  let n = entry_at(slot) + W_NODE;
  if (node_cache_word(n) != zoom) {
    return false;
  }
  let held = vec3<u32>(node_cache_word(n + 1u), node_cache_word(n + 2u), node_cache_word(n + 3u));
  return all(held == vec3<u32>(c));
}

/// `is_node`, with each word read as it stands now (`node_cache_now`).
fn is_node_now(slot: u32, zoom: u32, c: vec3<i32>) -> bool {
  let n = entry_at(slot) + W_NODE;
  return node_cache_now(n) == zoom
    && node_cache_now(n + 1u) == u32(c.x)
    && node_cache_now(n + 2u) == u32(c.y)
    && node_cache_now(n + 3u) == u32(c.z);
}

/// A hash of the node `c` at `zoom` asked at `zooms`. `zooms` holds the far zoom in its low
/// `ZOOM_BITS` and the near zoom above them.
fn pair_hash(zoom: u32, c: vec3<i32>, zooms: u32) -> u32 {
  let h = mix_hash(node_hash(zoom, c), zooms);
  return h ^ (h >> 16u);
}

/// Whether a key is a walk's claim, named or not (pxl-engine `node_cache::CLAIMED`).
fn is_claim(key: u32) -> bool {
  return key == CLAIMED || (key & CLAIM_MASK) == CLAIM_FAR;
}

/// How many times a walk reads a claim not yet named again before it gives up keeping the
/// node. The walk that claimed the slot names its claim right after.
const CLAIM_READS = 8u;

/// The stamp of an entry that its key keeps at `zooms` (`key_zooms`): a hash of its words,
/// stored above its kind (pxl-engine `node_cache::KIND_MASK`). Walks read and write the node
/// cache at once, so a walk takes a slot's entry only when the words it read hash to its stamp.
/// A cell kept by its node alone hashes as `CELL_FAR`, whole or not, as a walk marks it whole
/// in its key alone (`holds_cell`).
fn entry_stamp(kind: u32, zooms: u32, asset: vec2<u32>, inheritance: vec4<u32>, zoom: u32, c: vec3<i32>) -> u32 {
  var h = node_hash(zoom, c);
  h = mix_hash(h, select(zooms, CELL_FAR, (zooms & ((1u << ZOOM_BITS) - 1u)) == CELL_FAR));
  h = mix_hash(h, kind);
  h = mix_hash(h, asset.x);
  h = mix_hash(h, asset.y);
  h = mix_hash(h, inheritance.x);
  h = mix_hash(h, inheritance.y);
  h = mix_hash(h, inheritance.z);
  h = mix_hash(h, inheritance.w);
  return (h ^ (h >> 16u)) & ~KIND_MASK;
}

/// The node `c` at `zoom`, asked at `zooms` by a walk of this pass. It is answered from the
/// slot that keeps it, whichever walk filled that slot. Otherwise it is asked of the world and
/// kept (`keep`) under this pass's tag, and only then is what it inherits found: from the top
/// where `top` (`world_root`), and otherwise from what its parent inherited, `parent`
/// (`world_child`). What it hands its children is the slot's, when a slot kept it. An ask of
/// the world is counted at word `asked`, unless `asked` is `NONE`. When the pass may ask no
/// more (`may_ask`), nothing is asked and the walk is left there (`KIND_LEFT`).
fn ask_node(parent: vec4<u32>, top: bool, zoom: u32, c: vec3<i32>, zooms: u32, asked: u32) -> Answer {
  var kept = keeps(zoom, c);
  var first = 0u;
  if (kept) {
    first = bucket_of(zoom, c);
    for (var k = 0u; k < BUCKET; k += 1u) {
      let slot = first + k;
      let slot_key = node_cache_word(key_at(slot));
      if (slot_key != 0u && holds_for(slot_key, zoom, zooms) && is_node(slot, zoom, c)) {
        // The key is written last (`keep`), but a plain load may return the other words
        // from before that write, or from a later write over it. So an entry of any walk,
        // this pass's or an earlier frame's, is taken only when its words hash to its stamp
        // (`entry_stamp`). Nothing else keeps an entry torn this way from answering.
        let w = entry_at(slot);
        let at = w + W_INHERITANCE;
        let held = vec4<u32>(
          node_cache_word(at),
          node_cache_word(at + 1u),
          node_cache_word(at + 2u),
          node_cache_word(at + 3u),
        );
        let word = node_cache_word(w + W_KIND);
        let asset = vec2<u32>(node_cache_word(w + W_ASSET), node_cache_word(w + W_ASSET + 1u));
        if ((word & ~KIND_MASK) == entry_stamp(word & KIND_MASK, slot_key >> TAG_BITS, asset, held, zoom, c)) {
          return Answer(word & KIND_MASK, asset, held);
        }
        // The words are from two writes. They are left to the walk writing them, and this
        // walk does not keep its answer.
        kept = false;
        break;
      }
    }
  }
  if (!may_ask()) {
    return Answer(KIND_LEFT, vec2<u32>(0u), vec4<u32>(0u));
  }
  let a = world_answer(parent, top, zoom, c, zooms, asked);
  if (kept) {
    keep(first, zoom, c, zooms, walk_tag(), a);
  }
  return a;
}

/// Asks the world for the node `c` at `zoom` at `zooms`. What it inherits is found from the top
/// where `top`, and otherwise from `parent`. The ask is counted at word `asked`, unless `asked`
/// is `NONE`.
fn world_answer(parent: vec4<u32>, top: bool, zoom: u32, c: vec3<i32>, zooms: u32, asked: u32) -> Answer {
  if (asked != NONE && counts(asked)) {
    atomicAdd(&node_cache[asked], 1u);
  }
  var inheritance: vec4<u32>;
  if (top) {
    inheritance = world_root(zoom, c);
  } else {
    inheritance = world_child(parent, zoom, c);
  }
  return world_node(inheritance, zoom, c, zooms & ((1u << ZOOM_BITS) - 1u), zooms >> ZOOM_BITS, eye_cut(CARVE), eye_cut(HIDE));
}

/// Whether a slot of the bucket from `first` keeps the node `c` at `zoom` for a walk asking it
/// at `zooms` (`holds_for`), whichever walk and frame filled it. Its key is read as it stands
/// now.
fn holds_node(first: u32, zoom: u32, c: vec3<i32>, zooms: u32) -> bool {
  for (var k = 0u; k < BUCKET; k += 1u) {
    let slot = first + k;
    let key = node_cache_now(key_at(slot));
    if (key != 0u && !is_claim(key) && holds_for(key, zoom, zooms) && is_node_now(slot, zoom, c)) {
      return true;
    }
  }
  return false;
}

/// The zooms at which a key keeps the node at `zoom`, asked at `zooms` and answered `a`
/// (pxl-engine `node_cache::key_zooms`). A split is kept at the zooms asked. A cell is kept by
/// its node alone (`CELL_FAR`). A cell answered coarser than asked is kept whole, unless it is
/// void, which is kept at the zooms asked: a split can answer as a cell only as void.
fn key_zooms(zoom: u32, zooms: u32, a: Answer) -> u32 {
  if (a.kind == KIND_SPLIT) {
    return zooms;
  }
  if (zoom >= (zooms >> ZOOM_BITS)) {
    return CELL_FAR;
  }
  if (a.kind == KIND_LACKS || a.asset.x != VALUE_VOID || a.asset.y != 0u) {
    return CELL_FAR | (1u << ZOOM_BITS);
  }
  return zooms;
}

/// Whether the bucket from `first` keeps the node `c` at `zoom` by its node alone, its key read
/// as it stands now. When a slot does and this walk found the cell whole (`whole`), the slot's
/// key is marked whole if it is not yet (`holds_for`): the query's answer for a node coarser
/// than asked is whole for every walk once one walk finds it so.
fn holds_cell(first: u32, zoom: u32, c: vec3<i32>, whole: bool) -> bool {
  for (var k = 0u; k < BUCKET; k += 1u) {
    let slot = first + k;
    let key = node_cache_now(key_at(slot));
    if (key == 0u || is_claim(key) || ((key >> TAG_BITS) & ((1u << ZOOM_BITS) - 1u)) != CELL_FAR || !is_node_now(slot, zoom, c)) {
      continue;
    }
    if (whole) {
      atomicOr(&node_cache[key_at(slot)], 1u << (TAG_BITS + ZOOM_BITS));
    }
    return true;
  }
  return false;
}

/// Keeps the node `c` at `zoom` asked at `zooms`, answered `a`, under this pass's `tag`, in a
/// slot it claims (`claim`, `fill`), at the zooms its answer is kept at (`key_zooms`). Nothing
/// is kept when the cell is already kept by its node alone (`holds_cell`).
fn keep(first: u32, zoom: u32, c: vec3<i32>, zooms: u32, tag: u32, a: Answer) {
  let kz = key_zooms(zoom, zooms, a);
  if (kz != zooms && holds_cell(first, zoom, c, kz != CELL_FAR)) {
    return;
  }
  let slot = claim(first, zoom, c, kz, tag);
  if (slot != NONE) {
    fill(slot, zoom, c, kz, tag, a);
  }
}

/// Whether slot `slot`'s entry draws and was drawn for frame `frame` (pxl-engine
/// `record::drawn`): it has a face set, and was drawn this frame or the last, or by the eye's
/// last draw.
fn drawn(slot: u32, frame: u32) -> bool {
  if ((node_cache_now(state_at(slot)) & (RECORD_FACES | RECORD_ASKED)) == 0u) {
    return false;
  }
  let last = node_cache_now(drawn_at(slot));
  return last + 2u > frame || last == node_cache_word(NODE_CACHE_DRAWN);
}

/// Claims a slot of the bucket from `first` to keep the node `c` at `zoom` asked at `zooms`
/// under this pass's `tag` (pxl-engine node_cache.rs). It takes a free slot, the first after
/// the node's place at these zooms (`pair_hash`); otherwise a slot of an earlier frame that no
/// draw drew for this frame (`drawn`), the first after the node's own place (`node_hash`). It
/// returns the slot with its claim named, so no other walk keeps the node at these zooms beside
/// it. It returns `NONE` when a walk of this pass keeps or claims the node at these zooms, or
/// when no slot is free or of an earlier frame. A lost claim tries again over the bucket as it
/// then stands.
fn claim(first: u32, zoom: u32, c: vec3<i32>, zooms: u32, tag: u32) -> u32 {
  return claim_slot(first, zoom, c, zooms, tag).x;
}

/// `claim`, also returning the key the slot held before: the slot, then that key, which a claim
/// that is let go puts back (queue_ask.wgsl `ask_once`).
fn claim_slot(first: u32, zoom: u32, c: vec3<i32>, zooms: u32, tag: u32) -> vec2<u32> {
  let key = tag | (zooms << TAG_BITS);
  let h = pair_hash(zoom, c, zooms);
  let mine = (h & ~CLAIM_MASK) | CLAIM_FAR;
  let probe = h & (BUCKET - 1u);
  let victim = (node_hash(zoom, c) >> 28u) & (BUCKET - 1u);
  let frame = tag / PASSES;
  for (var tries = 0u; tries < BUCKET; tries += 1u) {
    // The slot to claim and the key read there. Every free slot comes before every slot of an
    // earlier frame.
    var to = NONE;
    var was = 0u;
    var best = 2u * BUCKET;
    for (var k = 0u; k < BUCKET; k += 1u) {
      let slot = first + k;
      var held = node_cache_now(key_at(slot));
      for (var r = 0u; r < CLAIM_READS && held == CLAIMED; r += 1u) {
        held = node_cache_now(key_at(slot));
      }
      if (held == CLAIMED || held == mine || (held == key && is_node_now(slot, zoom, c))) {
        return vec2<u32>(NONE, 0u);
      }
      var turn = 2u * BUCKET;
      if (held == 0u) {
        turn = (k - probe) & (BUCKET - 1u);
      } else if (!is_claim(held) && (held & ~KEY_ZOOMS) / PASSES < frame && !drawn(slot, frame)) {
        turn = BUCKET + ((k - victim) & (BUCKET - 1u));
      }
      if (turn < best) {
        best = turn;
        to = k;
        was = held;
      }
    }
    if (to == NONE) {
      return vec2<u32>(NONE, 0u);
    }
    let slot = first + to;
    if (node_cache_now(key_at(slot)) != was) {
      continue;
    }
    let got = atomicExchange(&node_cache[key_at(slot)], CLAIMED);
    if (got == was) {
      atomicStore(&node_cache[key_at(slot)], mine);
      return vec2<u32>(slot, was);
    }
    if (got != CLAIMED) {
      put_back(slot, got);
      if (got == mine) {
        return vec2<u32>(NONE, 0u);
      }
    }
  }
  return vec2<u32>(NONE, 0u);
}

/// Writes the node `c` at `zoom` asked at `zooms`, answered `a`, into slot `slot`, which its walk
/// claimed (`claim`). Every word is written, with nothing set yet to draw it (pxl-engine
/// `record`), and its key under `tag` is written last.
fn fill(slot: u32, zoom: u32, c: vec3<i32>, zooms: u32, tag: u32, a: Answer) {
  atomicStore(&node_cache[state_at(slot)], 0u);
  atomicStore(&node_cache[drawn_at(slot)], 0u);
  for (var face = 0u; face < 6u; face += 1u) {
    atomicStore(&node_cache[face_at(slot, face)], 0u);
  }
  let w = entry_at(slot);
  atomicStore(&node_cache[w + W_KIND], a.kind | entry_stamp(a.kind, zooms, a.asset, a.inheritance, zoom, c));
  atomicStore(&node_cache[w + W_ASSET], a.asset.x);
  atomicStore(&node_cache[w + W_ASSET + 1u], a.asset.y);
  atomicStore(&node_cache[w + W_INHERITANCE], a.inheritance.x);
  atomicStore(&node_cache[w + W_INHERITANCE + 1u], a.inheritance.y);
  atomicStore(&node_cache[w + W_INHERITANCE + 2u], a.inheritance.z);
  atomicStore(&node_cache[w + W_INHERITANCE + 3u], a.inheritance.w);
  atomicStore(&node_cache[w + W_NODE], zoom);
  atomicStore(&node_cache[w + W_NODE + 1u], u32(c.x));
  atomicStore(&node_cache[w + W_NODE + 2u], u32(c.y));
  atomicStore(&node_cache[w + W_NODE + 3u], u32(c.z));
  atomicStore(&node_cache[key_at(slot)], tag | (zooms << TAG_BITS));
}

/// A key's rank for `put_back`: `CLAIMED`, then a named claim, then a key. The walk that claimed
/// the slot writes each over the one before.
fn claim_rank(key: u32) -> u32 {
  if (key == CLAIMED) {
    return 0u;
  }
  return select(2u, 1u, is_claim(key));
}

/// Puts back into slot `slot` the key `got` that a lost claim took there, where it left
/// `CLAIMED`. It keeps the higher-ranked of that key and what it takes back in turn, as a walk
/// that lost the slot later may have taken a key that the claiming walk wrote in between.
fn put_back(slot: u32, got: u32) {
  var v = got;
  for (var i = 0u; i < 2u; i += 1u) {
    let x = atomicExchange(&node_cache[key_at(slot)], v);
    if (claim_rank(x) <= claim_rank(v)) {
      return;
    }
    v = x;
  }
}

/// The node `c` at `zoom`, asked at `zooms`, as the walk asks it (walk.wgsl `walk`). It is
/// `ask_node`, which reads the inherited words whatever `hands` says, with its asks counted
/// where the pass counts them (`asked_at`).
fn node_answer(parent: vec4<u32>, top: bool, zoom: u32, c: vec3<i32>, zooms: u32, hands: bool) -> Answer {
  return ask_node(parent, top, zoom, c, zooms, asked_at());
}

/// What a walk that asks returns when it ends at a lack: it stops there, as every node over the
/// lack was answered at the zooms asked.
fn lack_ends() -> u32 {
  return 2u;
}

/// Finds what the node inherits where a beam ray's walk ended without asking: from its
/// parent's, `parent`, or from the top where `top`. It writes that into the ray's line at
/// `line`.
fn hand_end(line: u32, parent: vec4<u32>, top: bool, zoom: u32, c: vec3<i32>) -> bool {
  if (top) {
    line_hand(line, zoom, world_root(zoom, c));
  } else {
    line_hand(line, zoom, world_child(parent, zoom, c));
  }
  return true;
}

/// Whether the observer whose walks count into lane word `at` counts this frame
/// (`node_cache::COUNT_ON`, `node_cache::lane_at`). Each observer's lanes lie `COUNT_WALKS`
/// words after the previous observer's, starting from the eye's.
fn counts(at: u32) -> bool {
  let observer = (at - EYE_BEAM_COUNT) / COUNT_WALKS;
  return ((node_cache_word(COUNT_ON) >> observer) & 1u) != 0u;
}

/// Adds the `visits` nodes a walk entered to word `at`, when its observer counts.
fn count_visits(at: u32, visits: u32) {
  if (counts(at)) {
    atomicAdd(&node_cache[at], visits);
  }
}
