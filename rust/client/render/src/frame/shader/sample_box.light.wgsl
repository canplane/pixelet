// pxl-render/sample_box.light.wgsl

// What sample_box.wgsl reads of the light: whether a sample stopped on its node whole, and
// where the faces' asks its samples open lie.

/// Whether sample `s` stopped on `k` whole, that is, on a node coarser than its band, the one
/// zoom a light's ray asks (pxl-engine `record::light_whole`).
fn sample_whole(s: vec2<u32>, k: Node) -> bool {
  return k.zoom < sample_band(s);
}

/// The light opens no faces' asks, because its slots read no face's word.
fn sample_asks() -> AskList {
  return AskList(0u, 0u, 0u);
}
