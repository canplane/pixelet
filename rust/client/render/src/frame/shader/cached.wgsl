// pxl-render/cached.wgsl

// How the walk that only reads the node cache answers a node (walk.wgsl). It uses no atomics
// and never asks the world, so a thread holds few words across its loop: on the Galaxy S23,
// atomic loads cost several times plain ones, and a pipeline with about 200 words live across a
// loop is refused. No thread of the pass writes what it reads, so an entry is read whole, as
// the pass before left it. A node that no slot keeps at the zooms asked ends the walk there,
// and the walk that asks the world (ask.wgsl) goes on from it. The exception is a split that a
// slot keeps only at other zooms: the walk goes into it (`KIND_BLIND`).

@group(0) @binding(9) var<storage, read> node_cache: array<u32>;

fn node_cache_word(k: u32) -> u32 {
  return node_cache[k];
}

/// The node `c` at `zoom`, asked at `zooms`, from the slot that keeps it for those zooms
/// (walk_node.wgsl `holds_for`): kept at those zooms, or as a cell by its node alone. What it
/// inherits is read only where `hands`. With no such slot, the answer is `KIND_LEFT`. A slot
/// that keeps a split at other zooms is gone into when the node is asked finer than itself
/// (`KIND_BLIND`). It hands on what it inherited of the clip levels only, not of the ways or the
/// shards, as a moved brick's may be gone (`Inheritance::of`). `parent` and `top` serve the walk
/// that asks (ask.wgsl) and are not read here.
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
      // A claimed key matches no zoom pair a walk asks and no cell, so it is passed over
      // (pxl-engine `node_cache::CLAIMED`).
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
      var inheritance = vec4<u32>(0u);
      if (hands) {
        let at = w + W_INHERITANCE;
        inheritance = vec4<u32>(node_cache[at], node_cache[at + 1u], node_cache[at + 2u], node_cache[at + 3u]);
      }
      let asset = vec2<u32>(node_cache[w + W_ASSET], node_cache[w + W_ASSET + 1u]);
      return Answer(node_cache[w + W_KIND] & KIND_MASK, asset, inheritance);
    }
    // A split beside the end of the service area is left to the walk that asks. The wall under
    // it lies in no box, and a walk that took a box would pass through it (`area::is_wall`).
    if (other != NONE && zoom < (zooms >> ZOOM_BITS) && !beside_end(node_cache[other + W_INHERITANCE + 1u])) {
      return Answer(KIND_BLIND, vec2<u32>(0u), vec4<u32>(node_cache[other + W_INHERITANCE], SHARD_UNKNOWN, LOOKUP_NONE, SLOT_NOTHING));
    }
  }
  return Answer(KIND_LEFT, vec2<u32>(0u), vec4<u32>(0u));
}

/// Whether a node's shard word `shard` knows of a shard beside the node that lies past the
/// service area.
fn beside_end(shard: u32) -> bool {
  return shard != SHARD_UNKNOWN && shard != SHARD_INSIDE;
}

/// What a walk over the node cache alone returns when it ends at a lack. The walk that asks
/// walks the ray again whole, as a split gone into without taking a box of it may have led it
/// there.
fn lack_ends() -> u32 {
  return ENDS_AGAIN;
}

/// What the node inherits where a beam ray's walk ended without asking. It cannot be found
/// without asking the world, so the walk is left there.
fn hand_end(line: u32, parent: vec4<u32>, top: bool, zoom: u32, c: vec3<i32>) -> bool {
  return false;
}
