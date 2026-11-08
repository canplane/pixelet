// pxl-render/record_hits.eye.wgsl

// The eye as its holes' stops take it (record_hits.wgsl): its stops past the beam rays'
// words, `ray::HITS_A_FRAME` at most (`beam::Grid::hits_at`); a stop's node from the eye's origin, whole
// where it is coarser than the eye asks it
// (`record::stopped_whole`); and its faces' asks among the grid's (`beam::Grid::hit_asks_at`).

fn hits_at() -> u32 {
  return eye_word(GRID + GRID_HITS_AT);
}

fn hits_cap() -> u32 {
  return HITS_A_FRAME;
}

fn hit_origin(r: u32) -> vec3<i32> {
  return eye_origin();
}

fn hit_whole(k: Node, r: u32) -> bool {
  return stopped_whole(k, eye_origin());
}

fn hit_asks() -> Asks {
  return Asks(RASTER_HIT_ASKED_AT, eye_word(GRID + GRID_HIT_ASKS_AT), HITS_A_FRAME);
}
