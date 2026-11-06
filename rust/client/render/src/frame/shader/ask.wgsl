// pxl-render/ask.wgsl

// A node as the walk that asks answers it (walk.wgsl): off the node cache, or asked of the
// world there and then and kept (pxl-engine `cast::walk`, `node_cache`). No stack: a node asked
// of the world finds what is handed it off its parent's where the walk holds them, else from
// the top (`world_root`). The node cache's words all atomic, as
// a slot's key is exchanged to claim it: no compare-and-swap, which Safari's WGSL compiler
// writes Metal for that does not compile (doc/experiment/20261103-shaders-on-two-phones). A
// pass names the tag its walks keep under (`WALK_PASS`), where its asks are counted
// (`asked_at`) and whether it may ask another (`may_ask`).

@group(0) @binding(9) var<storage, read_write> node_cache: array<atomic<u32>>;

fn node_cache_word(k: u32) -> u32 {
  return atomicLoad(&node_cache[k]);
}

/// Word `k` as it stands: read by an operation that writes it, as a plain atomic load may give
/// one a while old.
fn node_cache_now(k: u32) -> u32 {
  return atomicOr(&node_cache[k], 0u);
}

/// This pass's tag: the frame's number by `PASSES`, and the pass; a walk reads it once.
fn walk_tag() -> u32 {
  return PASSES * node_cache_word(FRAME) + WALK_PASS;
}

/// Whether the entry at word `w` is the node (`is_node`): its zoom first, its corner's three words
/// loaded together and compared at once only where that matches, as an entry of another node
/// mostly differs there already and every word is an atomic load.
fn is_node(w: u32, zoom: u32, c: vec3<i32>) -> bool {
  let n = w + W_NODE;
  if (node_cache_word(n) != zoom) {
    return false;
  }
  let held = vec3<u32>(node_cache_word(n + 1u), node_cache_word(n + 2u), node_cache_word(n + 3u));
  return all(held == vec3<u32>(c));
}

/// `is_node`, its words as they stand.
fn is_node_now(w: u32, zoom: u32, c: vec3<i32>) -> bool {
  let n = w + W_NODE;
  return node_cache_now(n) == zoom
    && node_cache_now(n + 1u) == u32(c.x)
    && node_cache_now(n + 2u) == u32(c.y)
    && node_cache_now(n + 3u) == u32(c.z);
}

/// The node `c` of `zoom` asked at `zooms`, far then near over `ZOOM_BITS`, hashed.
fn pair_hash(zoom: u32, c: vec3<i32>, zooms: u32) -> u32 {
  let h = mix_hash(node_hash(zoom, c), zooms);
  return h ^ (h >> 16u);
}

/// Whether a key is a walk's claim, named or not (pxl-engine `node_cache::CLAIMED`).
fn is_claim(key: u32) -> bool {
  return key == CLAIMED || (key & CLAIM_MASK) == CLAIM_FAR;
}

/// How often a walk reads again a claim not yet named before it keeps none: the walk that
/// claimed it names it next.
const CLAIM_READS = 8u;

/// The stamp of an entry its key keeps at `zooms`, far then near over `ZOOM_BITS`
/// (`key_zooms`), a hash of its words under its kind (pxl-engine `node_cache::KIND_MASK`): a
/// walk reads and writes at once here, so takes a slot only where the words it read hash to it.
/// A cell kept by its node alone hashes as `CELL_FAR`, whole or not, as a walk sets it whole
/// over its key alone (`cell_kept`).
fn entry_stamp(kind: u32, zooms: u32, asset: vec2<u32>, handed: vec4<u32>, zoom: u32, c: vec3<i32>) -> u32 {
  var h = node_hash(zoom, c);
  h = mix_hash(h, select(zooms, CELL_FAR, (zooms & ((1u << ZOOM_BITS) - 1u)) == CELL_FAR));
  h = mix_hash(h, kind);
  h = mix_hash(h, asset.x);
  h = mix_hash(h, asset.y);
  h = mix_hash(h, handed.x);
  h = mix_hash(h, handed.y);
  h = mix_hash(h, handed.z);
  h = mix_hash(h, handed.w);
  return (h ^ (h >> 16u)) & ~KIND_MASK;
}

/// The node `c` at `zoom`, asked at `zooms` by a walk of this pass: off the slot that keeps it,
/// of whichever walk; else of the world, and kept (`keep`) under this pass's tag, what is handed
/// it found there alone: from the top where `top` (`world_root`), else off what was handed its
/// parent, `parent` (`world_child`). What it hands its children: the slot's where one kept it.
/// One asked of the world counts at word `asked`, none where `NONE`; where the pass may ask no
/// more (`may_ask`), none is asked and the walk is left there (`KIND_LEFT`).
fn ask_node(parent: vec4<u32>, top: bool, zoom: u32, c: vec3<i32>, zooms: u32, asked: u32) -> Answer {
  var kept = keeps(zoom, c);
  var first = 0u;
  if (kept) {
    first = bucket_of(zoom, c);
    for (var k = 0u; k < BUCKET; k += 1u) {
      let w = ENTRIES_AT + (first + k) * ENTRY_WORDS;
      let slot_key = node_cache_word(w);
      if (slot_key != 0u && holds_for(slot_key, zoom, zooms) && is_node(w, zoom, c)) {
        // Its key is written last (`keep`), but a plain load may give the other words from
        // before that write, or from a write over it since: an entry of any walk, this pass's
        // as an earlier frame's, is taken only where its words hash to its stamp
        // (`entry_stamp`), which is all that keeps an entry torn so from answering.
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
        // Words of two writes: left to the walk that writes it.
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

/// The node `c` at `zoom` asked of the world at `zooms`, what is handed it found from the top
/// where `top`, else off `parent`; counted at word `asked`, none where `NONE`.
fn world_answer(parent: vec4<u32>, top: bool, zoom: u32, c: vec3<i32>, zooms: u32, asked: u32) -> Answer {
  if (asked != NONE && counts(asked)) {
    atomicAdd(&node_cache[asked], 1u);
  }
  var handed: vec4<u32>;
  if (top) {
    handed = world_root(zoom, c);
  } else {
    handed = world_child(parent, zoom, c);
  }
  return world_node(handed, zoom, c, zooms & ((1u << ZOOM_BITS) - 1u), zooms >> ZOOM_BITS, eye_cut(CARVE), eye_cut(HIDE));
}

/// Whether a slot of the bucket from `first` keeps the node `c` at `zoom` for a walk asking it
/// at `zooms` (`holds_for`), its key read as it stands, of whichever walk and frame.
fn kept_at(first: u32, zoom: u32, c: vec3<i32>, zooms: u32) -> bool {
  for (var k = 0u; k < BUCKET; k += 1u) {
    let w = ENTRIES_AT + (first + k) * ENTRY_WORDS;
    let key = node_cache_now(w);
    if (key != 0u && !is_claim(key) && holds_for(key, zoom, zooms) && is_node_now(w, zoom, c)) {
      return true;
    }
  }
  return false;
}

/// The zooms a key keeps the node of `zoom`, asked at `zooms` and answered `a`, at
/// (pxl-engine `node_cache::key_zooms`): a cell the query answers by its cell, by its node
/// alone (`CELL_FAR`), whole where it answered it coarser than asked, as no split answers a
/// cell but void; else the zooms asked.
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
/// as it stands: where one does and the walk found it whole, `whole`, set whole where it is not
/// (`holds_for`), as the query's answer for a node coarser than asked is whole whichever walk
/// finds it so.
fn cell_kept(first: u32, zoom: u32, c: vec3<i32>, whole: bool) -> bool {
  for (var k = 0u; k < BUCKET; k += 1u) {
    let w = ENTRIES_AT + (first + k) * ENTRY_WORDS;
    let key = node_cache_now(w);
    if (key == 0u || is_claim(key) || ((key >> TAG_BITS) & ((1u << ZOOM_BITS) - 1u)) != CELL_FAR || !is_node_now(w, zoom, c)) {
      continue;
    }
    if (whole) {
      atomicOr(&node_cache[w], 1u << (TAG_BITS + ZOOM_BITS));
    }
    return true;
  }
  return false;
}

/// Keeps the node `c` at `zoom` asked at `zooms`, answered `a`, under this pass's `tag`, in the
/// slot it claims (`claim`, `fill`), at the zooms its answer is kept at (`key_zooms`); none
/// where a cell is kept by its node alone already (`cell_kept`).
fn keep(first: u32, zoom: u32, c: vec3<i32>, zooms: u32, tag: u32, a: Answer) {
  let kz = key_zooms(zoom, zooms, a);
  if (kz != zooms && cell_kept(first, zoom, c, kz != CELL_FAR)) {
    return;
  }
  let w = claim(first, zoom, c, kz, tag);
  if (w != NONE) {
    fill(w, zoom, c, kz, tag, a);
  }
}

/// Whether the entry at word `w` draws and was drawn for frame `frame` (pxl-engine
/// `record::drawn`): a face set, and drawn this frame or the last, or by the eye's last draw.
fn drawn(w: u32, frame: u32) -> bool {
  if ((node_cache_now(w + W_STATE) & (RECORD_FACES | RECORD_ASKED)) == 0u) {
    return false;
  }
  let seen = node_cache_now(w + W_SEEN);
  return seen + 2u > frame || seen == node_cache_word(NODE_CACHE_DRAWN);
}

/// Claims a slot of the bucket from `first` to keep the node `c` at `zoom` asked at `zooms`
/// under this pass's `tag` (pxl-engine node_cache.rs): a free one, the first from the node's at
/// these zooms (`pair_hash`); else an earlier frame's that no draw drew for this frame
/// (`drawn`), the first from the node's own (`node_hash`). The slot's first word, its claim named, so no other walk keeps the node at
/// these zooms beside it; `NONE` where a walk of this pass keeps the node at these zooms or
/// claims it, or no slot is free or an earlier frame's. A lost claim tries again over the
/// bucket as it then stands.
fn claim(first: u32, zoom: u32, c: vec3<i32>, zooms: u32, tag: u32) -> u32 {
  return claim_slot(first, zoom, c, zooms, tag).x;
}

/// `claim`, with the key the slot held before it: the slot's word, then that key, which a claim
/// let go puts back (queue_ask.wgsl `ask_once`).
fn claim_slot(first: u32, zoom: u32, c: vec3<i32>, zooms: u32, tag: u32) -> vec2<u32> {
  let key = tag | (zooms << TAG_BITS);
  let h = pair_hash(zoom, c, zooms);
  let mine = (h & ~CLAIM_MASK) | CLAIM_FAR;
  let probe = h & (BUCKET - 1u);
  let victim = (node_hash(zoom, c) >> 28u) & (BUCKET - 1u);
  let frame = tag / PASSES;
  for (var tries = 0u; tries < BUCKET; tries += 1u) {
    // The slot it claims and the key it read there, a free slot's turn before every earlier
    // frame's.
    var to = NONE;
    var was = 0u;
    var best = 2u * BUCKET;
    for (var k = 0u; k < BUCKET; k += 1u) {
      let w = ENTRIES_AT + (first + k) * ENTRY_WORDS;
      var held = node_cache_now(w);
      for (var r = 0u; r < CLAIM_READS && held == CLAIMED; r += 1u) {
        held = node_cache_now(w);
      }
      if (held == CLAIMED || held == mine || (held == key && is_node_now(w, zoom, c))) {
        return vec2<u32>(NONE, 0u);
      }
      var turn = 2u * BUCKET;
      if (held == 0u) {
        turn = (k - probe) & (BUCKET - 1u);
      } else if (!is_claim(held) && (held & ~KEY_ZOOMS) / PASSES < frame && !drawn(w, frame)) {
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
    let w = ENTRIES_AT + (first + to) * ENTRY_WORDS;
    if (node_cache_now(w) != was) {
      continue;
    }
    let got = atomicExchange(&node_cache[w], CLAIMED);
    if (got == was) {
      atomicStore(&node_cache[w], mine);
      return vec2<u32>(w, was);
    }
    if (got != CLAIMED) {
      put_back(w, got);
      if (got == mine) {
        return vec2<u32>(NONE, 0u);
      }
    }
  }
  return vec2<u32>(NONE, 0u);
}

/// Writes the node `c` at `zoom` asked at `zooms`, answered `a`, in the slot at word `w` its
/// walk claimed (`claim`): every word, what draws it none (pxl-engine `record`), then its key
/// under `tag`, last.
fn fill(w: u32, zoom: u32, c: vec3<i32>, zooms: u32, tag: u32, a: Answer) {
  atomicStore(&node_cache[w + W_STATE], 0u);
  atomicStore(&node_cache[w + W_SEEN], 0u);
  for (var face = 0u; face < 6u; face += 1u) {
    atomicStore(&node_cache[w + W_FACES + face], 0u);
  }
  atomicStore(&node_cache[w + W_KIND], a.kind | entry_stamp(a.kind, zooms, a.asset, a.handed, zoom, c));
  atomicStore(&node_cache[w + W_ASSET], a.asset.x);
  atomicStore(&node_cache[w + W_ASSET + 1u], a.asset.y);
  atomicStore(&node_cache[w + W_INHERITANCE], a.handed.x);
  atomicStore(&node_cache[w + W_INHERITANCE + 1u], a.handed.y);
  atomicStore(&node_cache[w + W_INHERITANCE + 2u], a.handed.z);
  atomicStore(&node_cache[w + W_INHERITANCE + 3u], a.handed.w);
  atomicStore(&node_cache[w + W_NODE], zoom);
  atomicStore(&node_cache[w + W_NODE + 1u], u32(c.x));
  atomicStore(&node_cache[w + W_NODE + 2u], u32(c.y));
  atomicStore(&node_cache[w + W_NODE + 3u], u32(c.z));
  atomicStore(&node_cache[w], tag | (zooms << TAG_BITS));
}

/// A key as it ranks for `put_back`: `CLAIMED`, then a named claim, then a key, each written
/// over the one before by the walk that claimed the slot.
fn claim_rank(key: u32) -> u32 {
  if (key == CLAIMED) {
    return 0u;
  }
  return select(2u, 1u, is_claim(key));
}

/// Puts back at word `w` the key `got` a lost claim took there, `CLAIMED` in its place: the
/// highest of it and what it takes back in turn, as a walk that lost the slot later may have
/// taken the key the walk that claimed it wrote in between.
fn put_back(w: u32, got: u32) {
  var v = got;
  for (var i = 0u; i < 2u; i += 1u) {
    let x = atomicExchange(&node_cache[w], v);
    if (claim_rank(x) <= claim_rank(v)) {
      return;
    }
    v = x;
  }
}

/// The node `c` at `zoom`, asked at `zooms`, as the walk asks it (walk.wgsl `walk`): `ask_node`,
/// its handed words read whatever `hands`, its asks counted where the pass counts them
/// (`asked_at`).
fn node_answer(parent: vec4<u32>, top: bool, zoom: u32, c: vec3<i32>, zooms: u32, hands: bool) -> Answer {
  return ask_node(parent, top, zoom, c, zooms, asked_at());
}

/// What was handed the node a beam ray's walk ended at unasked, found here off its parent's,
/// `parent`, or from the top where `top`, and written in its line at `line`.
fn hand_end(line: u32, parent: vec4<u32>, top: bool, zoom: u32, c: vec3<i32>) -> bool {
  if (top) {
    line_hand(line, zoom, world_root(zoom, c));
  } else {
    line_hand(line, zoom, world_child(parent, zoom, c));
  }
  return true;
}

/// Whether the observer whose walks count into lane word `at` counts this frame
/// (`node_cache::COUNT_ON`, `node_cache::lane_at`): its lanes lie `COUNT_WALKS` an observer from
/// the eye's first.
fn counts(at: u32) -> bool {
  let observer = (at - EYE_BEAM_COUNT) / COUNT_WALKS;
  return ((node_cache_word(COUNT_ON) >> observer) & 1u) != 0u;
}

/// `visits` nodes a walk entered, counted at word `at` where its observer counts.
fn count_visits(at: u32, visits: u32) {
  if (counts(at)) {
    atomicAdd(&node_cache[at], visits);
  }
}
