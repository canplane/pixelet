// pxl-render/record_deferred.wgsl

// The hits whose stop met its key's claim in record_stops.wgsl `record_stops_cs`, put off and
// taken past that dispatch's end, every claim then written or let go, without claiming
// (`record_deferred_cs`): dispatched off `raster::DEFERRED_DISPATCH`'s buffer, which this pass
// does not bind, its count at `raster::DEFERRED_CNT`.

@compute @workgroup_size(HIT_LANES)
fn record_deferred_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x >= in_room(atomicLoad(&args[RASTER_DEFERRED_CNT]), hits_room())) {
    return;
  }
  let r = beams[defers_at() + id.x];
  _ = upsert(hit_kept(r), hit_stop(r), false, hit_asks());
}
