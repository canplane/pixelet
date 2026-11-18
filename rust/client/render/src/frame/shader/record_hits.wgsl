// pxl-render/frame/shader/record_hits.wgsl

// Writes a cast's holes' hits into the record, the eye's and the light's alike
// (pxl-engine record.rs, the third of its writers). The holes' queue keeps every hit, whatever
// it stopped on (queue_walk.wgsl and queue_last.wgsl, through queue.wgsl `keep_hit`;
// pxl-engine `beam::HIT_WORDS`).
// This pass runs once the queue is done, a thread a hit. Its dispatch reads the hits' count
// from a copy of the node cache's head (`node_cache::HITS_ARGS`; ray/raster.rs `rec_hits`),
// because the pass binds the node cache writable and a dispatch cannot take its arguments from
// a buffer it writes.
//
// `rec_hits_cs` finds each hit's entry and sets the face its ray entered (record_put.wgsl
// `observe_hit`). The first hit to enter a face asks that face's word, among the hits' asks
// (`beam::Grid::hit_asks_at`). Once the faces' pass has answered the asks, `rec_faces_cs`
// puts what lies around each hit's face at the hit's pixel.
//
// record_hits.eye.wgsl and record_hits.light.wgsl give what differs by cast: where its
// hits lie and how many there may be (`hits_at`, `hits_cap`), the origin a hit's node is held
// from (`hit_origin`), whether a hit is whole (`hit_whole`), and where its faces' asks lie
// (`hit_asks`).

@group(0) @binding(3) var rays: texture_storage_2d_array<r32uint, read_write>;

@compute @workgroup_size(HIT_LANE_CNT)
fn rec_hits_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x >= in_cap(node_cache_word(HITS_CNT), hits_cap())) {
    return;
  }
  observe_hit(hit_node(id.x), hit(id.x), hit_asks());
}

/// Puts what lies around each hit's face at the hit's pixel, where the pixel still waits for
/// it: its last ray word holds `HIT_WAIT` and the hit's number (pxl-engine `beam::HIT_WAIT`).
/// The word put is the entry's word for that face, which the faces' pass answered for this
/// hit or for an earlier one. It is 0 where no slot keeps the node.
@compute @workgroup_size(HIT_LANE_CNT)
fn rec_faces_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x >= in_cap(node_cache_word(HITS_CNT), hits_cap())) {
    return;
  }
  let k = hit_at(id.x);
  let px = beams[k + HIT_PX];
  let at = vec2<u32>(px & 0xffffu, px >> 16u);
  if (textureLoad(rays, at, 3u).x != (HIT_WAIT | id.x)) {
    return;
  }
  let h = hit_node(id.x);
  let p = hit(id.x);
  let slot = find(h.zoom, idx_of(h.corner, h.zoom, h.roots, vec3<i32>(0i)), p.asset, p.lacks);
  var word = 0u;
  if (slot != NO_ENTRY) {
    word = node_cache_word(face_at(slot, p.face));
  }
  textureStore(rays, at, 3u, vec4<u32>(word, 0u, 0u, 0u));
}
