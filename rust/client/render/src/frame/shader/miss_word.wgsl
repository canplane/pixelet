// pxl-render/miss_word.wgsl

// A missed pixel's ray and the words its walk leaves there, as the holes' queue takes and
// leaves them (queue.wgsl; grid.wgsl, hit.wgsl).

@group(0) @binding(3) var rays: texture_storage_2d_array<r32uint, read_write>;

/// Pixel `id`'s ray of rays `size` wide and high, through its middle off by the eye's jitter
/// (grid.wgsl `px_tan`).
fn px_aim(id: vec2<u32>, size: vec2<u32>) -> Aim {
  return eye_aim(px_tan(id, size));
}

/// Where pixel `id`'s ray aimed `aim` starts off the cell it goes through (grid.wgsl
/// `grid_cell`, resume.wgsl `resumed`).
fn px_start(aim: Aim, id: vec2<u32>, size: vec2<u32>) -> Start {
  return resumed(aim, cell_start(grid_cell(px_tan(id, size))), LINE_WORDS, NONE);
}

/// `px_start` at `t`, where its walk over the node cache left it (resume.wgsl `resumed_at`).
fn px_start_at(aim: Aim, id: vec2<u32>, size: vec2<u32>, t: f32) -> Start {
  return resumed_at(aim, cell_start(grid_cell(px_tan(id, size))), t, LINE_WORDS, NONE);
}

/// The face the ray aimed `aim` entered where walk `w` stopped: twice its axis, one more
/// where the ray runs up that axis (pxl-engine `ray::FACE_SHIFT`'s face).
fn face_of(aim: Aim, w: Walked) -> u32 {
  return 2u * w.axis + select(0u, 1u, aim.dir[w.axis] > 0.0);
}

/// The eighths of its cube the cell a walk stopped at keeps, 0 for the whole
/// (`answer::FILL_SHIFT`).
fn hit_fill(w: Walked) -> u32 {
  return (w.asset.x >> FILL_SHIFT) & (EIGHTHS - 1u);
}

/// Walk `w`'s stop at pixel `id`, entered through `face` (`beam::HIT_LACKS` set where it
/// lacks), put as stop `r` the record takes (`keep_hit`, `beam::HIT_WORDS`, record_stops.wgsl):
/// its node, the asset it stopped at, and the line its pixel's cell's rays resumed from, which
/// that face's word is asked off.
fn hit_put(r: u32, w: Walked, face: u32, id: vec2<u32>) {
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
  let cell = grid_cell(px_tan(id, textureDimensions(rays).xy));
  beams[k + HIT_LINE] = beams[cell_start(cell) + CELL_CORNER] * LINE_WORDS;
}

/// The ray's words where walk `w` ended, entered through `face`, `faces` what lies about that
/// face (`word`): where it met nothing, every word zero.
fn ray_word(w: Walked, face: u32, faces: u32) -> vec4<u32> {
  if (w.ends != 1u) {
    return vec4<u32>(0u);
  }
  return vec4<u32>(
    (w.asset.x & VAL_MASK) | (face << RAY_FACE_SHIFT) | (w.zoom << RAY_ZOOM_SHIFT) | (hit_fill(w) << RAY_FILL_SHIFT),
    bitcast<u32>(w.t),
    w.asset.y,
    faces,
  );
}

/// A ray's words written at pixel `id` (`write_ray`).
fn write_ray(id: vec2<u32>, word: vec4<u32>) {
  for (var i = 0u; i < 4u; i += 1u) {
    textureStore(rays, id, i, vec4<u32>(word[i], 0u, 0u, 0u));
  }
}
