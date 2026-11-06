// pxl-render/record_stops.eye.wgsl

// The eye as its holes' stops into its record take it (record_stops.wgsl): its stops past the
// beam rays' words and those put off, `HITS` at most (`beam::Grid::hits_at`, `defers_at`); a
// stop keyed at band 0 from the eye's origin, whole where it is coarser than the eye asks it
// (`record::stopped_whole`); and its faces' asks among the grid's (`beam::Grid::stop_asks_at`).

fn hits_at() -> u32 {
  return eye_word(GRID + GRID_HITS_AT);
}

fn defers_at() -> u32 {
  return eye_word(GRID + GRID_DEFERS_AT);
}

fn hits_room() -> u32 {
  return HITS;
}

fn hit_band(r: u32) -> u32 {
  return 0u;
}

fn hit_origin(r: u32) -> vec3<i32> {
  return eye_origin();
}

fn hit_whole(k: Kept, r: u32) -> bool {
  return stopped_whole(k, eye_origin());
}

fn hit_asks() -> Asks {
  return Asks(RASTER_STOP_ASKED_AT, eye_word(GRID + GRID_STOP_ASKS_AT), HITS);
}
