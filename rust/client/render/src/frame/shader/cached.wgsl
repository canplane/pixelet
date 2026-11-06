// pxl-render/cached.wgsl

// A node as the node cache alone answers it, for the walk that only reads it (walk.wgsl): no
// atomics and no query of the world, so a thread holds little across its loop
// (doc/experiment/20261103-the-walk-on-a-phone). What it reads no thread of its pass writes, so
// an entry is read whole, as the pass before left it. A node no slot keeps at the zooms asked
// leaves the walk there, for the walk that asks the world (ask.wgsl) to go on from.

@group(0) @binding(9) var<storage, read> node_cache: array<u32>;

fn node_cache_word(k: u32) -> u32 {
  return node_cache[k];
}

/// The node `c` at `zoom`, asked at `zooms`, far then near over `ZOOM_BITS`, off the slot that
/// keeps it for those zooms (walk_node.wgsl `holds_for`): at those zooms, or a cell by its node
/// alone; what was handed it read where `hands`; else `KIND_LEFT`. A slot keeping a split at
/// other zooms answers nothing, as pxl-engine's `Query` answers a node at the pair asked.
/// `parent` and `top` are the asking walk's (ask.wgsl), unread here.
fn node_answer(parent: vec4<u32>, top: bool, zoom: u32, c: vec3<i32>, zooms: u32, hands: bool) -> Answer {
  if (keeps(zoom, c)) {
    let node = vec4<u32>(zoom, u32(c.x), u32(c.y), u32(c.z));
    let first = bucket_of(zoom, c);
    for (var k = 0u; k < BUCKET; k += 1u) {
      let w = ENTRIES_AT + (first + k) * ENTRY_WORDS;
      let held = node_cache[w];
      // A claim reads as no zoom pair a walk asks and no cell (pxl-engine
      // `node_cache::CLAIMED`).
      if (held == 0u || !holds_for(held, zoom, zooms)) {
        continue;
      }
      let n = vec4<u32>(node_cache[w + W_NODE], node_cache[w + W_NODE + 1u], node_cache[w + W_NODE + 2u], node_cache[w + W_NODE + 3u]);
      if (any(n != node)) {
        continue;
      }
      var handed = vec4<u32>(0u);
      if (hands) {
        let at = w + W_INHERITANCE;
        handed = vec4<u32>(node_cache[at], node_cache[at + 1u], node_cache[at + 2u], node_cache[at + 3u]);
      }
      let asset = vec2<u32>(node_cache[w + W_ASSET], node_cache[w + W_ASSET + 1u]);
      return Answer(node_cache[w + W_KIND] & KIND_MASK, asset, handed);
    }
  }
  return Answer(KIND_LEFT, vec2<u32>(0u), vec4<u32>(0u));
}

/// What was handed the node a beam ray's walk ended at unasked: not found without asking the
/// world, so the walk is left there.
fn hand_end(line: u32, parent: vec4<u32>, top: bool, zoom: u32, c: vec3<i32>) -> bool {
  return false;
}
