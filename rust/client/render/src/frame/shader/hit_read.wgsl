// pxl-render/hit_read.wgsl

// An observer's holes' stops as what draws the node cache's entries takes them
// (record_hits.wgsl): the stops the holes' queue listed, a hit each (queue_walk.wgsl,
// queue_last.wgsl `keep_hit`, `beam::HIT_WORDS`), their count in the node cache's head
// (`node_cache::HITS_CNT`), and each one's node and stop as `observe_hit` takes them
// (record_put.wgsl).

@group(0) @binding(8) var<storage, read_write> beams: array<u32>;

/// Hit `r`'s node, its roots world-absolute from where its ray starts.
fn hit_node(r: u32) -> Node {
  return node_at(hit_at(r) + HIT_NODE, hit_origin(r));
}

/// Hit `r`'s stop: its asset, whether it lacks, the face its ray entered and its pixel's cell's
/// line.
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
