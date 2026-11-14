// pxl-render/frame/shader/hole.eye.wgsl

// The eye's holes as the holes' queue reads them (queue.wgsl): the pixels the eye's cast
// listed (cast.wgsl), where its pool and hits lie, a pixel's ray, and the words an ended
// walk leaves at the pixel.

fn eye_size() -> vec2<u32> {
  return textureDimensions(rays).xy;
}

fn pool_at() -> u32 {
  return eye_pool_at(eye_size());
}

/// Where the eye's hits lie (hit.wgsl), past the beam rays' words (`beam::Grid::hits_at`).
fn hits_at() -> u32 {
  return eye_word(EYE_GRID + GRID_HITS_AT);
}

/// Where the listed holes lie: their count, then their entries (`ray::holes_at`). There is
/// room for one a pixel.
fn holes_at() -> u32 {
  return eye_holes_at(eye_size());
}

fn holes_cap() -> u32 {
  let size = eye_size();
  return size.x * size.y;
}

fn item_aim(item: u32) -> Aim {
  return px_aim(hole_px(item), eye_size());
}

/// Where a hole's ray starts afresh: at the start of its cell.
fn item_start(item: u32, aim: Aim) -> Start {
  return px_start(aim, hole_px(item), eye_size());
}

/// The line the rays of pixel `id`'s cell resumed from, off which a hit's face word is asked
/// (miss_word.wgsl `hit_put`).
fn hit_line(id: vec2<u32>) -> u32 {
  let cell = grid_cell(px_tan(id, eye_size()));
  return beams[cell_start(cell) + CELL_CORNER] * LINE_WORDS;
}

/// Writes at pixel `at` the ray's words of walk `w`, which shows `shown` and entered through
/// `face`, with `faces` the word its hit takes from the record. A lack's hit takes no word
/// from the record, so what its pixel shows waits for none; a cell short of its cube shows a
/// box's faces (pxl-engine `face::BOX`).
fn hole_write(at: vec2<u32>, w: WalkEnd, shown: WalkEnd, face: u32, faces: u32) {
  var around = faces;
  if (w.ends == ENDS_LACK) {
    around = 0u;
  }
  if (shown.ends == ENDS_CELL && hit_fill(shown) != 0u) {
    around = FACE_BOX;
  }
  var word = ray_word(shown, face, around);
  // In the ray steps view, the steps of its beam ray and of its own walk.
  if (eye_word(EYE_STEPS) != 0u) {
    word.z = beam_step_cnt(grid_cell(px_tan(at, eye_size()))) + w.step;
  }
  write_ray(at, word);
}
