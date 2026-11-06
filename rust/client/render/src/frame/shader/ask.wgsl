// pxl-render/ask.wgsl

// A node as the walk that asks answers it (walk.wgsl): off the node cache, or asked of the
// world there and then and kept (pxl-engine `cast::walk`, `node_cache`). No stack: a node asked
// of the world finds what is handed it off its parent's where the walk holds them, else from
// the top (`world_root`). The node cache's words all atomic, as
// a slot's key is exchanged to claim it: no compare-and-swap, which Safari's WGSL compiler
// writes Metal for that does not compile (doc/experiment/20261103-shaders-on-two-phones). A
// pass names what its walks count and are tagged by (`WALK_PASS`, `VISITS_COUNT`, `ASKED_COUNT`).

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

/// The stamp of an entry asked at `zooms`, far then near over `ZOOM_BITS`, a hash of its
/// words under its kind (pxl-engine `node_cache::KIND_MASK`): a walk reads and writes at
/// once here, so takes a slot only where the words it read hash to it.
fn entry_stamp(kind: u32, zooms: u32, asset: vec2<u32>, handed: vec4<u32>, zoom: u32, c: vec3<i32>) -> u32 {
  var h = node_hash(zoom, c);
  h = mix_hash(h, zooms);
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
/// One asked of the world counts in `ASKED_COUNT` where `counts`.
fn ask_node(parent: vec4<u32>, top: bool, zoom: u32, c: vec3<i32>, zooms: u32, counts: bool) -> Answer {
  let want = zooms << TAG_BITS;
  var kept = keeps(zoom, c);
  var first = 0u;
  if (kept) {
    first = bucket_of(zoom, c);
    for (var k = 0u; k < BUCKET; k += 1u) {
      let w = ENTRIES_AT + (first + k) * ENTRY_WORDS;
      let slot_key = node_cache_word(w);
      if (slot_key != 0u && (slot_key & KEY_ZOOMS) == want && is_node(w, zoom, c)) {
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
        if ((word & ~KIND_MASK) == entry_stamp(word & KIND_MASK, zooms, asset, held, zoom, c)) {
          return Answer(word & KIND_MASK, asset, held);
        }
        // Words of two writes: left to the walk that writes it.
        kept = false;
        break;
      }
    }
  }
  if (counts && node_cache_word(COUNT_ON) != 0u) {
    atomicAdd(&node_cache[ASKED_COUNT], 1u);
  }
  var handed: vec4<u32>;
  if (top) {
    handed = world_root(zoom, c);
  } else {
    handed = world_child(parent, zoom, c);
  }
  let a = world_node(handed, zoom, c, zooms & ((1u << ZOOM_BITS) - 1u), zooms >> ZOOM_BITS, eye_cut(CARVE), eye_cut(HIDE));
  if (kept) {
    keep(first, zoom, c, zooms, walk_tag(), a);
  }
  return a;
}

/// Keeps the node `c` at `zoom` asked at `zooms`, answered `a`, under this pass's `tag`, in a slot
/// of the bucket from `first` (pxl-engine node_cache.rs): a free one, the first from the
/// node's at these zooms (`pair_hash`); else an earlier frame's, the first from the node's own
/// (`node_hash`); none where a walk
/// of this pass keeps the node at these zooms or claims it. A lost claim tries again over the
/// bucket as it then stands.
fn keep(first: u32, zoom: u32, c: vec3<i32>, zooms: u32, tag: u32, a: Answer) {
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
        return;
      }
      var turn = 2u * BUCKET;
      if (held == 0u) {
        turn = (k - probe) & (BUCKET - 1u);
      } else if (!is_claim(held) && (held & ~KEY_ZOOMS) / PASSES < frame) {
        turn = BUCKET + ((k - victim) & (BUCKET - 1u));
      }
      if (turn < best) {
        best = turn;
        to = k;
        was = held;
      }
    }
    if (to == NONE) {
      return;
    }
    let w = ENTRIES_AT + (first + to) * ENTRY_WORDS;
    if (node_cache_now(w) != was) {
      continue;
    }
    let got = atomicExchange(&node_cache[w], CLAIMED);
    if (got == was) {
      atomicStore(&node_cache[w], mine);
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
      atomicStore(&node_cache[w], key);
      return;
    }
    if (got != CLAIMED) {
      put_back(w, got);
      if (got == mine) {
        return;
      }
    }
  }
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
/// its handed words read whatever `hands`. A beam ray's asks are not counted, as a sample's and
/// a walk again's are (pxl-engine `count::ASKED`).
fn node_answer(parent: vec4<u32>, top: bool, zoom: u32, c: vec3<i32>, zooms: u32, hands: bool) -> Answer {
  return ask_node(parent, top, zoom, c, zooms, WALK_PASS != BEAM_PASS && WALK_PASS != LIGHT_BEAM_PASS);
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

/// The nodes a walk entered, counted in `VISITS_COUNT` where the walks count.
fn count_visits(w: Walked) {
  if (node_cache_word(COUNT_ON) != 0u) {
    atomicAdd(&node_cache[VISITS_COUNT], w.visits);
  }
}
