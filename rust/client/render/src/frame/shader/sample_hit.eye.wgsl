// pxl-render/sample_hit.eye.wgsl

// What sample_hit.wgsl reads of the eye: whether a sample stopped on its node whole, and where
// the faces' asks its samples open lie.

/// Whether sample `s` stopped on `k` whole, that is, on a node coarser than the zoom the eye
/// asks there from where the sample's ray starts (pxl-engine `record::ray_whole`).
fn sample_whole(s: vec2<u32>, k: Node) -> bool {
  return ray_whole(k, sample_aim(s).origin);
}

/// Where the samples' faces' asks lie (pxl-engine `beam::Grid::asks_at`), with room for one ask
/// a sample.
fn sample_asks() -> AskList {
  let n = sample_lattice();
  return AskList(RASTER_ASKED_AT, eye_word(EYE_GRID + GRID_ASKS_AT), n.x * n.y);
}
