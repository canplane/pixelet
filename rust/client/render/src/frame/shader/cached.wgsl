// pxl-render/cached.wgsl

// A node as the node cache alone answers it, for the walk that only reads it (walk.wgsl): no
// atomics and no query of the world, so a thread holds little across its loop
// (doc/experiment/20261103-the-walk-on-a-phone). What it reads no thread of its pass writes, so
// an entry is read whole, as the pass before left it. A node no slot keeps at the zooms asked
// leaves the walk there, for the walk that asks the world (ask.wgsl) to go on from, but for a
// split a slot keeps at other zooms or forgot, which the walk goes into (`KIND_BLIND`).

@group(0) @binding(9) var<storage, read> node_cache: array<u32>;

fn node_cache_word(k: u32) -> u32 {
  return node_cache[k];
}

/// The node `c` at `zoom`, asked at `zooms`, far then near over `ZOOM_BITS`, off the slot that
/// keeps it for those zooms (walk_node.wgsl `holds_for`): at those zooms, or a cell by its node
/// alone; what was handed it read where `hands`; else `KIND_LEFT`. A slot keeping a split at
/// other zooms, asked finer than itself, is gone into, what it was handed of the clip levels,
/// the ways and the shards not taken, as a moved brick's may be gone (`Inheritance::of`).
/// `parent` and `top` are the asking walk's (ask.wgsl), unread here.
fn node_answer(parent: vec4<u32>, top: bool, zoom: u32, c: vec3<i32>, zooms: u32, hands: bool) -> Answer {
  if (keeps(zoom, c)) {
    let node = vec4<u32>(zoom, u32(c.x), u32(c.y), u32(c.z));
    let first = bucket_of(zoom, c);
    var other = NONE;
    for (var k = 0u; k < BUCKET; k += 1u) {
      let held = node_cache[key_at(first + k)];
      if (held == 0u) {
        continue;
      }
      let w = entry_at(first + k);
      // A claim reads as no zoom pair a walk asks and no cell (pxl-engine
      // `node_cache::CLAIMED`).
      let kept = holds_for(held, zoom, zooms);
      if (!kept && (node_cache[w + W_KIND] & KIND_MASK) != KIND_SPLIT) {
        continue;
      }
      let n = vec4<u32>(node_cache[w + W_NODE], node_cache[w + W_NODE + 1u], node_cache[w + W_NODE + 2u], node_cache[w + W_NODE + 3u]);
      if (any(n != node)) {
        continue;
      }
      if (!kept) {
        other = w;
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
    // A split beside the service area's end is left: the wall under it lies in no box, which a
    // walk that took one goes past (`area::is_wall`).
    if (other != NONE && zoom < (zooms >> ZOOM_BITS) && !beside_end(node_cache[other + W_INHERITANCE + 1u])) {
      return Answer(KIND_BLIND, vec2<u32>(0u), vec4<u32>(node_cache[other + W_INHERITANCE], SHARD_UNKNOWN, LOOKUP_NONE, SLOT_NOTHING));
    }
  }
  return Answer(KIND_LEFT, vec2<u32>(0u), vec4<u32>(0u));
}

/// Whether what a node's shard word `shard` knows holds a shard about it past the service area.
fn beside_end(shard: u32) -> bool {
  return shard != SHARD_UNKNOWN && shard != SHARD_INSIDE;
}

/// What a walk over the node cache alone ends at a lack: walked again whole by the walk that
/// asks, as a split gone into taking no box of it may have led it there.
fn lack_ends() -> u32 {
  return ENDS_AGAIN;
}

/// What was handed the node a beam ray's walk ended at unasked: not found without asking the
/// world, so the walk is left there.
fn hand_end(line: u32, parent: vec4<u32>, top: bool, zoom: u32, c: vec3<i32>) -> bool {
  return false;
}
