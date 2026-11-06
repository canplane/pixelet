// pxl-render/record_stops.wgsl

// An observer's holes' stops into what draws the node cache's entries (record_put.wgsl
// `observe_stop`; pxl-engine record.rs), the eye's and the light's alike, each as its
// record_stops.eye.wgsl or record_stops.light.wgsl takes it: where its stops lie and their
// room (`hits_at`, `hits_room`), the origin a stop's node is held from (`hit_origin`), whether
// it stopped whole (`hit_whole`) and where its faces' asks lie (`hit_asks`). A thread a stop
// the holes' queue listed, every one whatever it stopped on (queue_walk.wgsl, queue_last.wgsl
// `keep_hit`, `beam::HIT_WORDS`), dispatched off their count copied out of the node cache's
// head (`node_cache::HITS_ARGS`; gpu/ray/raster.rs `stops`), as this pass binds the node cache
// writable, once the queue is done: each finds its node's entry and sets the face it entered,
// asking that face's word the first time, laid among the asks of the hits
// (`beam::Grid::stop_asks_at`), `HITS` at most (`record_stops_cs`). Then, once the faces' pass
// has answered their asks, what lies about each stop's face put at its pixel off its entry
// (`record_faces_cs`).

@group(0) @binding(3) var rays: texture_storage_2d_array<r32uint, read_write>;

@compute @workgroup_size(HIT_LANES)
fn record_stops_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x >= in_room(node_cache_word(HITS_CNT), hits_room())) {
    return;
  }
  observe_stop(hit_kept(id.x), hit_stop(id.x), hit_asks());
}

/// What lies about hit `r`'s face put at its pixel, where the pixel waits for it
/// (`beam::HIT_WAITS` and its number): its entry's word for that face's axis, which the faces'
/// pass answered or an earlier stop's did; none where no slot keeps its node.
@compute @workgroup_size(HIT_LANES)
fn record_faces_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x >= in_room(node_cache_word(HITS_CNT), hits_room())) {
    return;
  }
  let k = hit_at(id.x);
  let px = beams[k + HIT_PX];
  let at = vec2<u32>(px & 0xffffu, px >> 16u);
  if (textureLoad(rays, at, 3u).x != (HIT_WAITS | id.x)) {
    return;
  }
  let h = hit_kept(id.x);
  let p = hit_stop(id.x);
  let slot = find(h.zoom, idx_of(h.corner, h.zoom, h.roots, vec3<i32>(0i)), p.asset, p.lacks);
  var word = 0u;
  if (slot != NO_ENTRY) {
    word = node_cache_word(entry_at(slot) + W_FACES + p.face);
  }
  textureStore(rays, at, 3u, vec4<u32>(word, 0u, 0u, 0u));
}
