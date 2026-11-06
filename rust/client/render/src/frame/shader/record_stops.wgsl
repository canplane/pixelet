// pxl-render/record_stops.wgsl

// An observer's holes' stops into its record of voxels (record_put.wgsl `upsert`; pxl-engine
// record.rs), the eye's and the light's alike, each as its record_stops.eye.wgsl or
// record_stops.light.wgsl takes it: where its stops and those put off lie and their room
// (`hits_at`, `defers_at`, `hits_room`), the band and the origin a stop is keyed from
// (`hit_band`, `hit_origin`), whether it stopped whole (`hit_whole`) and where its faces'
// asks lie (`hit_asks`). A thread a stop the holes' queue listed, every one whatever it stopped on
// (queue_walk.wgsl, queue_last.wgsl `keep_hit`, `beam::HIT_WORDS`), dispatched off their count
// (`node_cache::HITS_ARGS`) once the queue is done: each finds its node's entry or claims and
// writes it,
// marks it seen and sets the face it entered, asking that face's word the first time, laid
// among the record's asks of the hits (`beam::Grid::stop_asks_at`), `HITS` at most
// (`record_stops_cs`). A hit whose stop met its key's claim is put off, its number listed
// (`beam::Grid::defers_at`, its count at `raster::DEFERRED_CNT`) and the dispatch over them
// raised in a buffer of its own (`deferred`), which the pass that takes them past the
// dispatch's end reads its count from and does not bind (record_deferred.wgsl). Then, once the
// faces' pass has answered their asks, what lies about each stop's face put at its pixel off
// its entry (`record_faces_cs`).

@group(0) @binding(3) var rays: texture_storage_2d_array<r32uint, read_write>;

/// The dispatch over the hits put off (pxl-engine `raster::DEFERRED_DISPATCH`), across raised
/// here, read by record_deferred.wgsl's dispatch.
@group(0) @binding(24) var<storage, read_write> deferred: array<atomic<u32>, 3>;

@compute @workgroup_size(HIT_LANES)
fn record_stops_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x >= in_room(node_cache[HITS_CNT], hits_room())) {
    return;
  }
  if (upsert(hit_kept(id.x), hit_stop(id.x), true, hit_asks()) == MET_CLAIM) {
    // Put off: a hit a place at most, so the list holds every one; the dispatch over them, a
    // buffer of its own, raised by the place that opens a workgroup.
    let place = args_add(RASTER_DEFERRED_CNT, 1u);
    if (place < hits_room()) {
      beams[defers_at() + place] = id.x;
      if (place % HIT_LANES == 0u) {
        atomicMax(&deferred[0], workgroups(place + 1u, hits_room(), HIT_LANES));
      }
    }
  }
}

/// What lies about hit `r`'s face put at its pixel, where the pixel waits for it
/// (`beam::HIT_WAITS` and its number): its entry's word for that face's axis, which the faces'
/// pass answered or an earlier stop's did; none where no entry holds its node (the record full).
@compute @workgroup_size(HIT_LANES)
fn record_faces_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x >= in_room(node_cache[HITS_CNT], hits_room())) {
    return;
  }
  let k = hit_at(id.x);
  let px = beams[k + HIT_PX];
  let at = vec2<u32>(px & 0xffffu, px >> 16u);
  if (textureLoad(rays, at, 3u).x != (HIT_WAITS | id.x)) {
    return;
  }
  let slot = find(hit_kept(id.x));
  var word = 0u;
  if (slot != NO_ENTRY) {
    word = record_word(record_at(slot) + R_FACES + (beams[k + HIT_FACE] & ~HIT_LACKS));
  }
  textureStore(rays, at, 3u, vec4<u32>(word, 0u, 0u, 0u));
}
