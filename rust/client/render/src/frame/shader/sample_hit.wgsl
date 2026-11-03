// pxl-render/sample_hit.wgsl

// The samples' walk over the observed pyramid alone (walk.wgsl). One it leaves is
// listed for `sample_cs`; one that ends is put as that puts it (pxl-shader beam.rs
// `sample_put`).

/// `sample_cs`'s dispatch, then how many are listed: only the list's own atomics.
@group(0) @binding(10) var<storage, read_write> left: array<atomic<u32>, 4>;

@compute @workgroup_size(8, 8, 1)
fn sample_hit_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let samples = vec2<u32>(eye_word(GRID + 9u), eye_word(GRID + 10u));
  if (id.x >= samples.x || id.y >= samples.y) {
    return;
  }
  let span_n = vec2<u32>(eye_word(GRID + 5u), eye_word(GRID + 6u));
  let cell_samples = SAMPLE * eye_word(GRID + 16u);
  let spacing = eye_f32(GRID) / f32(cell_samples);
  let tx = -eye_f32(TAN) + (f32(id.x) - f32(NEAR_SAMPLES)) * spacing;
  let ty = eye_f32(TAN + 1u) - (f32(id.y) - f32(NEAR_SAMPLES)) * spacing;
  let cell = min(
    (max(id.xy, vec2<u32>(NEAR_SAMPLES)) - vec2<u32>(NEAR_SAMPLES)) / cell_samples,
    span_n - vec2<u32>(1u),
  );
  let w = walk(vec2<f32>(tx, ty), cell, NONE);
  if (w.ends == 0u || w.ends == 4u) {
    let place = atomicAdd(&left[3], 1u);
    if (place % 64u == 0u) {
      atomicAdd(&left[0], 1u);
    }
    beams[eye_word(GRID + 17u) + 1u + place] = id.x | (id.y << 16u);
    return;
  }
  let base = eye_word(GRID + 12u) + (id.y * samples.x + id.x) * SAMPLE_WORDS;
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
  beams[base + SAMPLE_VISITS] = w.visits;
}
