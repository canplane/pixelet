// pxl-render/frame/shader/miss_word.wgsl

// A missed pixel's ray, and the words its walk leaves at the pixel and among the hits, as the
// holes' queue takes and leaves them (queue.wgsl; grid.wgsl, hit.wgsl).

@group(0) @binding(3) var rays: texture_storage_2d_array<r32uint, read_write>;

fn px_aim(id: vec2<u32>, size: vec2<u32>) -> Aim {
  return eye_aim(px_tan(id, size));
}

/// Where the ray `aim` of pixel `id` starts, resumed from the camera grid's cell it goes
/// through (grid.wgsl `grid_cell`, resume.wgsl `resumed`).
fn px_start(aim: Aim, id: vec2<u32>, size: vec2<u32>) -> Start {
  return resumed(aim, cell_start(grid_cell(px_tan(id, size))), LINE_WORDS, NONE);
}

/// The face that the ray `aim` entered where walk `w` stopped: twice its axis, plus one where
/// the ray runs up that axis (pxl-engine `ray::FACE_SHIFT`).
fn face_of(aim: Aim, w: WalkEnd) -> u32 {
  return 2u * w.axis + select(0u, 1u, aim.dir[w.axis] > 0.0);
}

/// How many eighths of its cube the cell a walk stopped at keeps, with 0 for the whole cube
/// (`answer::FILL_SHIFT`).
fn hit_fill(w: WalkEnd) -> u32 {
  return (w.asset.x >> FILL_SHIFT) & (EIGHTH_CNT - 1u);
}

/// Puts the hit of walk `w` at pixel `id` as hit `r`, which the record takes (`keep_hit`,
/// `beam::HIT_WORDS`, record_hits.wgsl). The ray entered through `face`, which has
/// `beam::HIT_LACK` set where the hit lacks. The hit holds its node, the asset it stopped
/// at, and the line its cast gives it (hole.eye.wgsl, hole.light.wgsl `hit_line`).
fn hit_put(r: u32, w: WalkEnd, face: u32, id: vec2<u32>) {
  let k = hit_at(r);
  beams[k + HIT_NODE + NODE_ZOOM] = w.zoom;
  beams[k + HIT_NODE + NODE_CORNER] = w.corner.x;
  beams[k + HIT_NODE + NODE_CORNER + 1u] = w.corner.y;
  beams[k + HIT_NODE + NODE_CORNER + 2u] = w.corner.z;
  beams[k + HIT_NODE + NODE_ROOTS] = u32(w.roots.x);
  beams[k + HIT_NODE + NODE_ROOTS + 1u] = u32(w.roots.y);
  beams[k + HIT_NODE + NODE_ROOTS + 2u] = u32(w.roots.z);
  beams[k + HIT_FACE] = face;
  beams[k + HIT_PX] = (id.y << 16u) | id.x;
  beams[k + HIT_ASSET] = w.asset.x;
  beams[k + HIT_ASSET + 1u] = w.asset.y;
  beams[k + HIT_LINE] = hit_line(id);
}

/// The ray's words where walk `w` ended, entered through `face`, with `faces` saying what lies
/// around that face (pxl-engine `face::word`). Where the ray met nothing, every word is zero.
fn ray_word(w: WalkEnd, face: u32, faces: u32) -> vec4<u32> {
  if (w.ends != ENDS_CELL) {
    return vec4<u32>(0u);
  }
  return vec4<u32>(
    (w.asset.x & VAL_MASK) | (face << RAY_FACE_SHIFT) | (w.zoom << RAY_ZOOM_SHIFT) | (hit_fill(w) << RAY_FILL_SHIFT),
    bitcast<u32>(w.t),
    w.asset.y,
    faces,
  );
}

fn write_ray(id: vec2<u32>, word: vec4<u32>) {
  for (var i = 0u; i < 4u; i += 1u) {
    textureStore(rays, id, i, vec4<u32>(word[i], 0u, 0u, 0u));
  }
}
