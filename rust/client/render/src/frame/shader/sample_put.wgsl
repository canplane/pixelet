// pxl-render/sample_put.wgsl

// A sample's hit as the raster reads it (pxl-engine `beam::SAMPLE_WORDS`).

/// The ended walk `w` put at word `base`, the nodes it and its beam ray visited `visits`
/// (pxl-engine `beam::SAMPLE_WORDS`): `SAMPLE_NONE` where it went on to its reach.
fn sample_put(base: u32, w: Walked, visits: u32) {
  let at = base + SAMPLE_NODE;
  beams[at + NODE_ZOOM] = select(w.zoom, SAMPLE_NONE, w.ends == 3u);
  beams[at + NODE_CORNER] = w.corner.x;
  beams[at + NODE_CORNER + 1u] = w.corner.y;
  beams[at + NODE_CORNER + 2u] = w.corner.z;
  beams[at + NODE_ROOTS] = u32(w.roots.x);
  beams[at + NODE_ROOTS + 1u] = u32(w.roots.y);
  beams[at + NODE_ROOTS + 2u] = u32(w.roots.z);
  beams[base + SAMPLE_LACKS] = select(0u, 1u, w.ends == 2u);
  beams[base + SAMPLE_ASSET] = select(w.asset.x, 0u, w.ends == 3u);
  beams[base + SAMPLE_ASSET + 1u] = select(w.asset.y, 0u, w.ends == 3u);
  beams[base + SAMPLE_AXIS] = select(w.axis, 0u, w.ends == 3u);
  beams[base + SAMPLE_VISITS] = visits;
}
