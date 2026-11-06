// pxl-render/record_stops.wgsl

// The holes' stops into the eye's record of voxels (record_put.wgsl `upsert`; pxl-engine
// record.rs), a thread a stop the holes' queue listed, every one whatever it stopped on
// (queue_walk.wgsl, queue_last.wgsl `keep_hit`, `beam::HIT_WORDS`), dispatched off their count
// (`node_cache::HITS_ARGS`) once the queue is done: each finds its node's entry or claims and
// writes it,
// marks it seen and sets the face it entered, asking that face's word the first time, laid
// among the record's asks of the hits (`beam::Grid::stop_asks_at`), `HITS` at most
// (`record_stops_cs`). A hit whose stop met its key's claim is put off, its number listed
// (`beam::Grid::defers_at`, its count and dispatch at `raster::DEFERRED_AT`), and taken past
// the dispatch's end, every claim then written or let go, without claiming
// (`record_deferred_cs`). Then, once the faces' pass has answered their asks, what lies about
// each stop's face put at its pixel off its entry (`record_faces_cs`).

@group(0) @binding(3) var rays: texture_storage_2d_array<r32uint, read_write>;

@group(0) @binding(8) var<storage, read_write> beams: array<u32>;
@group(0) @binding(9) var<storage, read> node_cache: array<u32>;

/// Hit `r`'s node as the record keys it.
fn hit_kept(r: u32) -> Kept {
  return kept_at(hit_at(r) + HIT_NODE, 0u);
}

/// Hit `r`'s stop: its asset, whether it lacks, the face its ray entered and its pixel's cell's
/// line.
fn hit_stop(r: u32) -> Stop {
  let k = hit_at(r);
  let face = beams[k + HIT_FACE];
  return Stop(vec2<u32>(beams[k + HIT_ASSET], beams[k + HIT_ASSET + 1u]), (face & HIT_LACKS) != 0u, face & ~HIT_LACKS, beams[k + HIT_LINE]);
}

fn hit_asks() -> Asks {
  return Asks(RASTER_STOP_ASKED_AT, eye_word(GRID + GRID_STOP_ASKS_AT), HITS);
}

@compute @workgroup_size(HIT_LANES)
fn record_stops_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x >= in_room(node_cache[HITS_CNT], HITS)) {
    return;
  }
  if (upsert(hit_kept(id.x), hit_stop(id.x), true, hit_asks()) == MET_CLAIM) {
    // Put off: a hit a place at most, so the list holds every one; the dispatch over them
    // raised by the place that opens a workgroup.
    let place = args_add(RASTER_DEFERRED_CNT, 1u);
    if (place < HITS) {
      beams[eye_word(GRID + GRID_DEFERS_AT) + place] = id.x;
      if (place % HIT_LANES == 0u) {
        args_most(RASTER_DEFERRED_ARGS, workgroups(place + 1u, HITS, HIT_LANES));
      }
    }
  }
}

@compute @workgroup_size(HIT_LANES)
fn record_deferred_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x >= in_room(atomicLoad(&args[RASTER_DEFERRED_CNT]), HITS)) {
    return;
  }
  let r = beams[eye_word(GRID + GRID_DEFERS_AT) + id.x];
  _ = upsert(hit_kept(r), hit_stop(r), false, hit_asks());
}

/// What lies about hit `r`'s face put at its pixel, where the pixel waits for it
/// (`beam::HIT_WAITS` and its number): its entry's word for that face's axis, which the faces'
/// pass answered or an earlier stop's did; none where no entry holds its node (the record full).
@compute @workgroup_size(HIT_LANES)
fn record_faces_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x >= in_room(node_cache[HITS_CNT], HITS)) {
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
    word = record_word(record_at(slot) + R_FACES + (beams[k + HIT_FACE] & ~HIT_LACKS) / 2u);
  }
  textureStore(rays, at, 3u, vec4<u32>(word, 0u, 0u, 0u));
}
