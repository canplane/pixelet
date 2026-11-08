// pxl-render/sample_box.eye.wgsl

// What sample_box.wgsl reads of the eye: whether a sample stopped on its node whole, and where
// the faces' asks its samples open lie.

/// Whether sample `s` stopped on `k` whole, that is, on a node coarser than the zoom the eye
/// asks there from where the sample's ray starts (pxl-engine `record::stopped_whole`).
fn sample_whole(s: vec2<u32>, k: Node) -> bool {
  return stopped_whole(k, sample_aim(s).origin);
}

/// Where the samples' faces' asks lie (pxl-engine `beam::Grid::asks_at`), with room for one ask
/// a sample.
fn sample_asks() -> Asks {
  let n = sample_lattice();
  return Asks(RASTER_ASKED_AT, eye_word(GRID + GRID_ASKS_AT), n.x * n.y);
}
