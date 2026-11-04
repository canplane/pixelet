// pxl-render/miss_word.wgsl

// A missed pixel's ray and the words its walk leaves there, as the walk over the observed
// pyramid alone (miss_hit.wgsl) and the walk that writes it (miss.wgsl) take and leave them
// (pxl-shader cast.rs `px_tan`, `word`, `write_ray`, `keep_hit`).

@group(0) @binding(3) var rays: texture_storage_2d_array<r32uint, read_write>;

/// Pixel `id`'s ray of rays `size` wide and high, through its middle off by the eye's jitter
/// (`px_tan`), and where it starts off its cell (`Grid::cell`, `resumed`).
struct Px {
  aim: Aim,
  start: Start,
}

fn px(id: vec2<u32>, size: vec2<u32>) -> Px {
  let tan_at = px_tan(id, size);
  let aim = eye_aim(tan_at);
  return Px(aim, resumed(aim, cell_start(grid_cell(tan_at)), LINE_WORDS, NONE));
}

/// The face the ray aimed `aim` entered where walk `w` stopped (`world::face_of`).
fn face_of(aim: Aim, w: Walked) -> u32 {
  return 2u * w.axis + select(0u, 1u, aim.dir[w.axis] > 0.0);
}

/// The eighths of its cube the cell a walk stopped at keeps, 0 for the whole
/// (`answer::FILL_SHIFT`).
fn hit_fill(w: Walked) -> u32 {
  return (w.asset.x >> FILL_SHIFT) & (EIGHTHS - 1u);
}

/// Walk `w`'s hit at pixel `id`, entered through `face`, put as hit `r` for what lies about
/// that face to be asked once a hit (`keep_hit`, `beam::HIT_WORDS`).
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
}

/// The ray's words where walk `w` ended, the nodes it and its beam ray visited `visits`,
/// entered through `face`, `faces` what lies about that face (`word`): where it met nothing,
/// the first two zero.
fn ray_word(w: Walked, visits: u32, face: u32, faces: u32) -> vec4<u32> {
  if (w.ends != 1u) {
    return vec4<u32>(0u, 0u, visits, 0u);
  }
  let steps = eye_word(ORIGIN + 3u) != 0u;
  return vec4<u32>(
    (w.asset.x & VAL_MASK) | (face << RAY_FACE_SHIFT) | (w.zoom << RAY_ZOOM_SHIFT) | (hit_fill(w) << RAY_FILL_SHIFT),
    bitcast<u32>(w.t),
    select(w.asset.y, visits, steps),
    faces,
  );
}

/// A ray's words written at pixel `id` (`write_ray`).
fn write_ray(id: vec2<u32>, word: vec4<u32>) {
  for (var i = 0u; i < 4u; i += 1u) {
    textureStore(rays, id, i, vec4<u32>(word[i], 0u, 0u, 0u));
  }
}
