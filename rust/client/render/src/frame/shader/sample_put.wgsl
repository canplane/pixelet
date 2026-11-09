// pxl-render/sample_put.wgsl

// Where a sample's walk ended, written into the words a sample leaves for the record's stops
// (pxl-engine `beam::SAMPLE_WORDS`), which sample_hit.wgsl reads.

/// Writes the ended walk `w` at word `base`. A walk that went on to its reach (`ends` 3) writes
/// `SAMPLE_NONE` as its zoom and no asset or axis. A walk that ended in a lack (`ends` 2) sets
/// `SAMPLE_LACK` in the lack word (walk_node.wgsl `WalkEnd`), which holds the steps it took
/// above it.
fn sample_put(base: u32, w: WalkEnd) {
  let at = base + SAMPLE_NODE;
  beams[at + NODE_ZOOM] = select(w.zoom, SAMPLE_NONE, w.ends == 3u);
  beams[at + NODE_CORNER] = w.corner.x;
  beams[at + NODE_CORNER + 1u] = w.corner.y;
  beams[at + NODE_CORNER + 2u] = w.corner.z;
  beams[at + NODE_ROOTS] = u32(w.roots.x);
  beams[at + NODE_ROOTS + 1u] = u32(w.roots.y);
  beams[at + NODE_ROOTS + 2u] = u32(w.roots.z);
  beams[base + SAMPLE_LACKS] = select(0u, SAMPLE_LACK, w.ends == 2u) | (w.step << SAMPLE_STEPS_SHIFT);
  beams[base + SAMPLE_ASSET] = select(w.asset.x, 0u, w.ends == 3u);
  beams[base + SAMPLE_ASSET + 1u] = select(w.asset.y, 0u, w.ends == 3u);
  beams[base + SAMPLE_AXIS] = select(w.axis, 0u, w.ends == 3u);
}
