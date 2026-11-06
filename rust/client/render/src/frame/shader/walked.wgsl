// pxl-render/walked.wgsl

// Where the eye's walked pixels' lists lie among the beam rays' words, past the camera grid's
// (pxl-engine `beam::Grid::walked_at`, `ray::walked_words`), room for every pixel drawn: the
// pixels the reading walk marked, after their count (cast.wgsl `cast_cs`), which the holes'
// queue takes (queue.wgsl) where the eye casts; then the two lists of the pixels its queue left
// parked, one taken by a frame that runs the queue alone, the other filled by it
// (queue_last.wgsl `unreached`).

/// The word of the marked pixels' count (`ray::WALKED_CNT`).
fn walked_cnt_at() -> u32 {
  return eye_word(GRID + GRID_WALKED_AT) + WALKED_CNT;
}

/// The word of the marked pixel at place `k` (`ray::WALKED_LIST`).
fn walked_list_at(k: u32) -> u32 {
  return eye_word(GRID + GRID_WALKED_AT) + WALKED_LIST + k;
}

/// The first word of pixel `p`'s parked state, of a target `size`, at its index in the eye's
/// holes' region, the last words of the beam rays' `words` (pxl-engine `ray::region_words`,
/// queue.wgsl `park`).
fn eye_state_at(p: vec2<u32>, size: vec2<u32>, words: u32) -> u32 {
  return words - size.x * size.y * STATE_WORDS + (p.y * size.x + p.x) * STATE_WORDS;
}

/// The word saying which list the eye's holes' queue takes, of a target `drawn` pixels: the
/// walked list, or an owed list (pxl-engine `ray::holes_from_at`, `ray::FROM_OWED`).
fn holes_from_at(drawn: u32) -> u32 {
  return eye_word(GRID + GRID_WALKED_AT) + WALKED_LIST + drawn;
}

/// The first word, its count, of owed list `k` of a target `drawn` pixels
/// (`ray::owed_list_at`).
fn owed_list_at(drawn: u32, k: u32) -> u32 {
  return holes_from_at(drawn) + 1u + k * (WALKED_LIST + drawn);
}

/// The first word, its count, of the list the eye's queue takes where the word at
/// `holes_from_at` says `taken`.
fn holes_list_at(drawn: u32, taken: u32) -> u32 {
  if (taken < FROM_OWED) {
    return walked_cnt_at();
  }
  return owed_list_at(drawn, taken - FROM_OWED);
}

/// The first word, its count, of the owed list the eye's queue lists what it leaves parked in,
/// where it takes `taken`: the one it does not take (`ray::owed_into`).
fn owed_into_at(drawn: u32, taken: u32) -> u32 {
  return owed_list_at(drawn, select(0u, 1u, taken == FROM_OWED));
}
