// pxl-render/miss_hit.wgsl

// The pixels `cast_cs` listed, their walk over the observed pyramid alone (walk.wgsl).
// One it leaves stays marked and is listed for `miss_list_cs` (miss.wgsl); one that ends is
// written as that writes it (miss_word.wgsl).

/// `miss_list_cs`'s dispatch and how many are listed, then the kept hits' dispatch
/// and how many (`pyramid::HITS_ARGS`, copied there after).
@group(0) @binding(10) var<storage, read_write> left: array<atomic<u32>, 8>;

/// A hit kept for what lies about its face to be asked once a hit (`keep_hit`).
fn keep_hit(w: Walked, face: u32, id: vec2<u32>) -> u32 {
  let r = atomicAdd(&left[7], 1u);
  if (r >= HITS) {
    return 0u;
  }
  if (r % HIT_LANES == 0u) {
    atomicMax(&left[4], r / HIT_LANES + 1u);
  }
  hit_put(r, w, face, id);
  return HIT_WAITS | r;
}

@compute @workgroup_size(64, 1, 1)
fn miss_hit_cs(
  @builtin(workgroup_id) group: vec3<u32>,
  @builtin(local_invocation_id) lane: vec3<u32>,
) {
  let k = (group.y * MISS_ACROSS + group.x) * MISS_LANES + lane.x;
  if (k >= pyramid[MARKS_CNT] || k >= MARKS) {
    return;
  }
  let listed = pyramid[MARKS_AT + k];
  let id = vec2<u32>(listed & 0xffffu, listed >> 16u);
  let size = textureDimensions(rays);
  if (id.x >= size.x || id.y >= size.y) {
    return;
  }
  let p = px(id, size);
  let w = walk(p.aim, p.start, NONE);
  if (w.ends == 0u || w.ends == 4u) {
    let place = atomicAdd(&left[3], 1u);
    if (place % (MISS_LANES * MISS_ACROSS) == 0u) {
      atomicMax(&left[1], place / (MISS_LANES * MISS_ACROSS) + 1u);
    }
    beams[eye_word(GRID + GRID_MISSES_AT) + 1u + place] = listed;
    return;
  }
  let face = face_of(p.aim, w);
  var faces = 0u;
  if (w.ends == 1u) {
    faces = select(FACE_BOX, keep_hit(w, face, id), hit_fill(w) == 0u);
  }
  write_ray(id, ray_word(w, w.visits, face, faces));
}
