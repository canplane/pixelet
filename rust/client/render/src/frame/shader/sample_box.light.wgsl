// pxl-render/sample_box.light.wgsl

// The light as its samples' stops into its record take it (sample_box.wgsl): whether a sample
// stopped on its node whole, where their faces' asks lie, none, as its slots read no face's
// word, and which samples a dispatch takes together.

/// Whether sample `s` stopped on `k` whole, coarser than its band (`record::light_whole`).
fn sample_whole(s: vec2<u32>, k: Kept) -> bool {
  return k.zoom < k.band;
}

/// No room: the light's slots read no face's word.
fn sample_asks() -> Asks {
  return Asks(0u, 0u, 0u);
}

/// Whether the dispatch that takes sample `s` takes sample `b`: surely where both lie in one
/// tile, as the dispatch takes the samples of the tiles listed whole (light_list.wgsl
/// `light_item`), the tiles `TILE`-aligned in the map's slots. A sample of a tile not listed
/// holds what an earlier cast left, so a sample after it heads its own run.
fn sample_with(s: vec2<u32>, b: vec2<u32>) -> bool {
  let side = LIGHT_TILE / LIGHT_SAMPLE;
  return all(s / side == b / side);
}
