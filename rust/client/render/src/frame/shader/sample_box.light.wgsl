// pxl-render/sample_box.light.wgsl

// The light as its samples' stops take it (sample_box.wgsl): whether a sample stopped on its
// node whole, and where their faces' asks lie, none, as its slots read no face's word.

/// Whether sample `s` stopped on `k` whole, coarser than its band (`record::light_whole`).
fn sample_whole(s: vec2<u32>, k: Kept) -> bool {
  return k.zoom < sample_band(s);
}

/// No room: the light's slots read no face's word.
fn sample_asks() -> Asks {
  return Asks(0u, 0u, 0u);
}
