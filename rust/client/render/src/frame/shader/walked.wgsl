// pxl-render/walked.wgsl

// The eye's holes among the beam rays' words, past the camera grid's (pxl-engine
// `beam::Grid::walked_at`, `ray::holes_words`): a bit a drawn pixel, in squares of `CAST_SIDE`
// pixels of `SQUARE_WORDS` words each, row by row. The walked bits, the pixels the last cast
// marked (cast.wgsl `cast_cs`), which the light's receivers read (light_mark.wgsl); the holes the
// cast listed (queue.wgsl); and the pool their rays are parked in between the run's walks
// (`ray::pool_at`).

/// The walked bits' first word.
fn walked_at() -> u32 {
  return eye_word(GRID + GRID_WALKED_AT);
}

/// The squares of a target `size` across and down (`ray::squares`).
fn squares(size: vec2<u32>) -> vec2<u32> {
  return (size + vec2<u32>(CAST_SIDE - 1u)) / CAST_SIDE;
}

/// The walked bits' words of a target `size` (`ray::walked_words`).
fn walked_words(size: vec2<u32>) -> u32 {
  let s = squares(size);
  return s.x * s.y * SQUARE_WORDS;
}

/// Pixel `p`'s word among one set of the holes' bits of a target `size`, and its bit in it
/// (`ray::hole_bit`).
fn hole_bit(p: vec2<u32>, size: vec2<u32>) -> vec2<u32> {
  let square = (p.y / CAST_SIDE) * squares(size).x + p.x / CAST_SIDE;
  let bit = (p.y % CAST_SIDE) * CAST_SIDE + p.x % CAST_SIDE;
  return vec2<u32>(square * SQUARE_WORDS + bit / 32u, bit % 32u);
}

/// The pixel of bit `bit` of word `word` of a set of the holes' bits of a target `size`.
fn bit_px(word: u32, bit: u32, size: vec2<u32>) -> vec2<u32> {
  let square = word / SQUARE_WORDS;
  let k = (word % SQUARE_WORDS) * 32u + bit;
  let across = squares(size).x;
  return vec2<u32>((square % across) * CAST_SIDE + k % CAST_SIDE, (square / across) * CAST_SIDE + k / CAST_SIDE);
}

/// The listed holes' count, their entries after it (`ray::holes_at`).
fn eye_holes_at(size: vec2<u32>) -> u32 {
  return walked_at() + walked_words(size);
}

/// The eye's pool's first word, its count (`ray::pool_at`, `ray::POOL_CNT`).
fn eye_pool_at(size: vec2<u32>) -> u32 {
  return eye_holes_at(size) + 1u + size.x * size.y;
}
