// pxl-render/miss_hit.wgsl

// The pixels `cast_cs` listed, their walk over the observed pyramid alone (walk.wgsl).
// One it leaves stays marked and is listed for `miss_list_cs`; one that ends is
// written as that writes it (pxl-shader cast.rs `walk_missed`, `word`, `keep_hit`).

@group(0) @binding(3) var rays: texture_storage_2d_array<r32uint, read_write>;
/// `miss_list_cs`'s dispatch and how many are listed, then the kept hits' dispatch
/// and how many (`pyramid::HITS_ARGS`, copied there after).
@group(0) @binding(10) var<storage, read_write> left: array<atomic<u32>, 8>;

/// A hit kept for what lies about its face to be asked once a hit (`keep_hit`).
fn keep_hit(w: Walked, id: vec2<u32>) -> u32 {
  let r = atomicAdd(&left[7], 1u);
  if (r >= HITS) {
    return 0u;
  }
  if (r % HIT_LANES == 0u) {
    atomicMax(&left[4], r / HIT_LANES + 1u);
  }
  let k = eye_word(GRID + 14u) + r * HIT_WORDS;
  beams[k + HIT_NODE + NODE_ZOOM] = w.zoom;
  beams[k + HIT_NODE + NODE_CORNER] = w.corner.x;
  beams[k + HIT_NODE + NODE_CORNER + 1u] = w.corner.y;
  beams[k + HIT_NODE + NODE_CORNER + 2u] = w.corner.z;
  beams[k + HIT_NODE + NODE_ROOTS] = u32(w.roots.x);
  beams[k + HIT_NODE + NODE_ROOTS + 1u] = u32(w.roots.y);
  beams[k + HIT_NODE + NODE_ROOTS + 2u] = u32(w.roots.z);
  beams[k + HIT_FACE] = w.face;
  beams[k + HIT_PX] = (id.y << 16u) | id.x;
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
  // Where its ray goes through the pixel (`px_tan`) and its cell (`Grid::cell`).
  let jitter = vec2<f32>(eye_f32(JITTER), eye_f32(JITTER + 1u));
  let tan_xy = vec2<f32>(eye_f32(TAN), eye_f32(TAN + 1u));
  let ndc = (vec2<f32>(id) + vec2<f32>(0.5) + jitter) / vec2<f32>(size) * 2.0 - vec2<f32>(1.0);
  let tan_at = vec2<f32>(ndc.x, -ndc.y) * tan_xy;
  let span_n = vec2<u32>(eye_word(GRID + 5u), eye_word(GRID + 6u));
  let s = eye_f32(GRID);
  let cell = min(
    vec2<u32>(max(floor(vec2<f32>(tan_at.x + tan_xy.x, tan_xy.y - tan_at.y) / s), vec2<f32>(0.0))),
    span_n - vec2<u32>(1u),
  );
  let w = walk(tan_at, cell, NONE);
  if (w.ends == 0u || w.ends == 4u) {
    let place = atomicAdd(&left[3], 1u);
    if (place % (MISS_LANES * MISS_ACROSS) == 0u) {
      atomicMax(&left[1], place / (MISS_LANES * MISS_ACROSS) + 1u);
    }
    beams[eye_word(GRID + 18u) + 1u + place] = listed;
    return;
  }
  var word = vec4<u32>(0u, 0u, w.visits, 0u);
  if (w.ends == 1u) {
    let fill = (w.asset.x >> FILL_SHIFT) & (EIGHTHS - 1u);
    let faces = select(FACE_BOX, keep_hit(w, id), fill == 0u);
    let steps = eye_word(ORIGIN + 3u) != 0u;
    word = vec4<u32>(
      (w.asset.x & VAL_MASK) | (w.face << RAY_FACE_SHIFT) | (w.zoom << RAY_ZOOM_SHIFT) | (fill << RAY_FILL_SHIFT),
      bitcast<u32>(w.t),
      select(w.asset.y, w.visits, steps),
      faces,
    );
  }
  for (var i = 0u; i < 4u; i += 1u) {
    textureStore(rays, id, i, vec4<u32>(word[i], 0u, 0u, 0u));
  }
}
