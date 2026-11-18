// pxl-render/frame/shader/hit_read.wgsl

// How the record (record_hits.wgsl) reads a cast's holes' hits. The walks of the holes'
// queue (queue_walk.wgsl, queue_last.wgsl) list each hit in `beam::HIT_WORDS` words
// through `keep_hit` (queue.wgsl) and count them in the node cache's head
// (`node_cache::HITS_CNT`). A hit is read here as the node and the `Hit` that `observe_hit`
// takes (record_put.wgsl).

@group(0) @binding(8) var<storage, read_write> beams: array<u32>;

/// The node of hit `r`. The hit holds the node's roots from where its ray starts, and they are
/// made world-absolute here.
fn hit_node(r: u32) -> Node {
  return node_at(hit_at(r) + HIT_NODE, hit_origin(r));
}

fn hit(r: u32) -> Hit {
  let k = hit_at(r);
  let face = beams[k + HIT_FACE];
  return Hit(
    vec2<u32>(beams[k + HIT_ASSET], beams[k + HIT_ASSET + 1u]),
    (face & HIT_LACK) != 0u,
    hit_whole(hit_node(r), r),
    face & ~HIT_LACK,
    beams[k + HIT_LINE],
  );
}
