// pxl-render/frame/shader/walked.wgsl

// Where the eye's holes lie among the beam rays' words, past the camera grid's (pxl-engine
// `beam::Grid::walked_at`, `ray::holes_words`). Three parts follow one another. The walked bits
// come first, one bit a drawn pixel, in squares `CAST_SIDE` pixels a side of `SQUARE_WORDS`
// words each, row by row. They mark the pixels the last cast sent to walk (cast.wgsl
// `cast_cs`), and the light's receivers read them (light_mark.wgsl). Then come the holes the
// cast listed (queue.wgsl), and last the pool their rays are parked in between the run's walks
// (`ray::pool_at`).

fn walked_at() -> u32 {
  return eye_word(EYE_GRID + GRID_WALKED_AT);
}

/// How many squares a target of `size` pixels holds across and down (pxl-engine
/// `ray::square_cnt`).
fn square_cnt(size: vec2<u32>) -> vec2<u32> {
  return (size + vec2<u32>(CAST_SIDE - 1u)) / CAST_SIDE;
}

/// How many words the walked bits of a target of `size` pixels take (pxl-engine
/// `ray::walked_words`).
fn walked_words(size: vec2<u32>) -> u32 {
  let s = square_cnt(size);
  return s.x * s.y * SQUARE_WORDS;
}

/// The word of pixel `p` in one set of hole bits for a target of `size` pixels, and its bit in
/// that word (pxl-engine `ray::hole_bit`).
fn hole_bit(p: vec2<u32>, size: vec2<u32>) -> vec2<u32> {
  let square = (p.y / CAST_SIDE) * square_cnt(size).x + p.x / CAST_SIDE;
  let bit = (p.y % CAST_SIDE) * CAST_SIDE + p.x % CAST_SIDE;
  return vec2<u32>(square * SQUARE_WORDS + bit / 32u, bit % 32u);
}

/// The pixel that bit `bit` of word `word` stands for, in a set of hole bits for a target of
/// `size` pixels; the inverse of `hole_bit`.
fn bit_px(word: u32, bit: u32, size: vec2<u32>) -> vec2<u32> {
  let square = word / SQUARE_WORDS;
  let k = (word % SQUARE_WORDS) * 32u + bit;
  let across = square_cnt(size).x;
  return vec2<u32>((square % across) * CAST_SIDE + k % CAST_SIDE, (square / across) * CAST_SIDE + k / CAST_SIDE);
}

/// The word of the listed holes' count, with their entries after it (pxl-engine
/// `ray::holes_at`).
fn eye_holes_at(size: vec2<u32>) -> u32 {
  return walked_at() + walked_words(size);
}

/// The first word of the eye's pool (pxl-engine `ray::pool_at`).
fn eye_pool_at(size: vec2<u32>) -> u32 {
  return eye_holes_at(size) + 1u + size.x * size.y;
}
