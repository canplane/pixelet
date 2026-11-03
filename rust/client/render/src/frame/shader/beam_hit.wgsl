// pxl-render/beam_hit.wgsl

// The beam rays' walk over the observed pyramid alone (walk.wgsl), its line kept as
// it goes. One it leaves is listed for `beam_cs`, which writes what it asks; one that
// ends is left as that leaves it (pxl-shader observer.rs `beam`, beam.rs `Leaving`).

/// `beam_cs`'s dispatch, then how many are listed: only the list's own atomics.
@group(0) @binding(10) var<storage, read_write> left: array<atomic<u32>, 4>;

@compute @workgroup_size(8, 8, 1)
fn beam_hit_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let corners = vec2<u32>(eye_word(GRID + 7u), eye_word(GRID + 8u));
  if (id.x >= corners.x || id.y >= corners.y) {
    return;
  }
  let past = f32(eye_word(GRID + 4u));
  let s = eye_f32(GRID);
  let tx = -eye_f32(TAN) + (f32(id.x) - past) * s;
  let ty = eye_f32(TAN + 1u) - (f32(id.y) - past) * s;
  let idx = id.y * corners.x + id.x;
  let base = idx * LINE_WORDS;
  let w = walk(vec2<f32>(tx, ty), vec2<u32>(0u), base);
  if (w.ends == 0u || w.ends == 4u) {
    let place = atomicAdd(&left[3], 1u);
    if (place % 64u == 0u) {
      atomicAdd(&left[0], 1u);
    }
    beams[eye_word(GRID + 19u) + 1u + place] = id.x | (id.y << 16u);
    return;
  }
  let at = base + LINE_NODE;
  beams[at + NODE_ZOOM] = w.zoom;
  beams[at + NODE_CORNER] = w.corner.x;
  beams[at + NODE_CORNER + 1u] = w.corner.y;
  beams[at + NODE_CORNER + 2u] = w.corner.z;
  beams[at + NODE_ROOTS] = u32(w.roots.x);
  beams[at + NODE_ROOTS + 1u] = u32(w.roots.y);
  beams[at + NODE_ROOTS + 2u] = u32(w.roots.z);
  beams[base + LINE_VISITS] = w.visits;
  // How far the rays about it start (`Grid::hand_at`, `Grid::hand_dist`).
  beams[corners.x * corners.y * LINE_WORDS + idx] = bitcast<u32>(w.t * eye_f32(GRID + 1u));
}
