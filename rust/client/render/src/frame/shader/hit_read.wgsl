// pxl-render/hit_read.wgsl

// How the record (record_hits.wgsl) reads an observer's holes' stops. The holes' queue lists
// each stop as a hit of `beam::HIT_WORDS` words (queue_walk.wgsl, queue_last.wgsl `keep_hit`)
// and counts them in the node cache's head (`node_cache::HITS_CNT`). A hit is read here as the
// node and the stop that `observe_hit` takes (record_put.wgsl).

@group(0) @binding(8) var<storage, read_write> beams: array<u32>;

/// The node of hit `r`. The hit holds the node's roots from where its ray starts, and they are
/// made world-absolute here.
fn hit_node(r: u32) -> Node {
  return node_at(hit_at(r) + HIT_NODE, hit_origin(r));
}

/// The stop of hit `r`, as record_put.wgsl `Hit` lays it out.
fn hit(r: u32) -> Hit {
  let k = hit_at(r);
  let face = beams[k + HIT_FACE];
  return Hit(
    vec2<u32>(beams[k + HIT_ASSET], beams[k + HIT_ASSET + 1u]),
    (face & HIT_LACKS) != 0u,
    hit_whole(hit_node(r), r),
    face & ~HIT_LACKS,
    beams[k + HIT_LINE],
  );
}
