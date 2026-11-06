// pxl-render/sample_box.eye.wgsl

// The eye as its samples' stops take it (sample_box.wgsl): whether a sample stopped on its node
// whole, and where their faces' asks lie.

/// Whether sample `s` stopped on `k` whole, coarser than the eye asks it from where the ray
/// starts (`record::stopped_whole`).
fn sample_whole(s: vec2<u32>, k: Kept) -> bool {
  return stopped_whole(k, sample_aim(s).origin);
}

/// The samples' faces' asks (`beam::Grid::asks_at`), a sample's room each.
fn sample_asks() -> Asks {
  let n = sample_lattice();
  return Asks(RASTER_ASKED_AT, eye_word(GRID + GRID_ASKS_AT), n.x * n.y);
}
