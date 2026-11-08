// pxl-render/record_hits.eye.wgsl

// What record_hits.wgsl reads of the eye. The eye's holes' stops lie past the beam rays' words,
// at most `ray::HITS_A_FRAME` of them (pxl-engine `beam::Grid::hits_at`). A stop's node is held
// from the eye's origin, and the stop is whole where the node is coarser than the zoom the eye
// asks there (`record::stopped_whole`). The faces' asks the stops open lie among the grid's
// words (`beam::Grid::hit_asks_at`).

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
