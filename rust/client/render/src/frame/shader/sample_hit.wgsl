// pxl-render/sample_hit.wgsl

// The samples' walk over the node cache alone (walk.wgsl), either observer's
// (sample_eye.wgsl, sample_light.wgsl). One it leaves is listed for `sample_cs`, which goes on
// from where it was left (pxl-engine `beam::SAMPLE_LEFT_T`); one that ends is put as that puts it
// (sample_put.wgsl).

/// `sample_cs`'s dispatch, then how many are listed: only the list's own atomics.
@group(0) @binding(10) var<storage, read_write> left: array<atomic<u32>, 4>;

@compute @workgroup_size(SAMPLE_SIDE, SAMPLE_SIDE, 1)
fn sample_hit_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let s = sample_item(id.xy);
  let lattice = sample_lattice();
  if (s.x >= lattice.x || s.y >= lattice.y) {
    return;
  }
  let base = sample_word(s);
  // One not needed meets nothing, so draws no box (`sample_box`).
  if (!sample_needed(s)) {
    beams[base + SAMPLE_NODE + NODE_ZOOM] = SAMPLE_NONE;
    return;
  }
  let aim = sample_aim(s);
  let w = walk(aim, sample_start(aim, s), NONE, vec2<f32>(0.0));
  if (w.ends == 4u) {
    beams[base + SAMPLE_LEFT_T] = bitcast<u32>(w.t);
    beams[base + SAMPLE_VISITS] = w.visits;
    let place = atomicAdd(&left[3], 1u);
    if (place % (SAMPLE_SIDE * SAMPLE_SIDE) == 0u) {
      atomicAdd(&left[0], 1u);
    }
    beams[left_at() + 1u + place] = s.x | (s.y << 16u);
    return;
  }
  sample_put(base, w, w.visits);
}
