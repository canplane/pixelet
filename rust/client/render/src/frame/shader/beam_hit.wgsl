// pxl-render/beam_hit.wgsl

// The beam rays' walk over the node cache alone (walk.wgsl), its line kept as
// it goes. One it leaves is listed for `beam_cs` (beam.wgsl), which writes what it asks; one
// that ends is left as that leaves it.

/// `beam_cs`'s dispatch, then how many are listed: only the list's own atomics.
@group(0) @binding(10) var<storage, read_write> left: array<atomic<u32>, 4>;

@compute @workgroup_size(CORNER_SIDE, CORNER_SIDE, 1)
fn beam_hit_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let corners = vec2<u32>(eye_word(GRID + GRID_CORNERS), eye_word(GRID + GRID_CORNERS + 1u));
  if (id.x >= corners.x || id.y >= corners.y) {
    return;
  }
  let past = f32(eye_word(GRID + GRID_PAST));
  let s = eye_f32(GRID + GRID_S);
  let tx = -eye_f32(TAN) + (f32(id.x) - past) * s;
  let ty = eye_f32(TAN + 1u) - (f32(id.y) - past) * s;
  let idx = id.y * corners.x + id.x;
  let base = idx * LINE_WORDS;
  let w = walk(eye_aim(vec2<f32>(tx, ty)), root_start(), base);
  if (w.ends == 4u) {
    let place = atomicAdd(&left[3], 1u);
    if (place % (CORNER_SIDE * CORNER_SIDE) == 0u) {
      atomicAdd(&left[0], 1u);
    }
    beams[eye_word(GRID + GRID_CORNERS_AT) + 1u + place] = id.x | (id.y << 16u);
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
  beams[eye_word(GRID + GRID_HANDS_AT) + idx] = bitcast<u32>(w.t * eye_f32(GRID + GRID_KEEP));
}
