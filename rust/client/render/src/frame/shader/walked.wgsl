// pxl-render/walked.wgsl

// Where the eye's walked pixels' lists lie among the beam rays' words, past the camera grid's
// (pxl-engine `beam::Grid::walked_at`, `ray::walked_words`), room for every pixel drawn: the
// pixels the reading walk marked, after their count (cast.wgsl `cast_cs`); then those the
// walk over the node cache left, after theirs (miss_hit.wgsl), `drawn` the pixels drawn.

/// The word of the marked pixels' count (`ray::WALKED_CNT`).
fn walked_cnt_at() -> u32 {
  return eye_word(GRID + GRID_WALKED_AT) + WALKED_CNT;
}

/// The word of the marked pixel at place `k` (`ray::WALKED_LIST`).
fn walked_list_at(k: u32) -> u32 {
  return eye_word(GRID + GRID_WALKED_AT) + WALKED_LIST + k;
}

/// The word of the left pixels' count, the left pixel at place `k` past it
/// (`ray::walked_left_at`).
fn walked_left_at(drawn: u32) -> u32 {
  return eye_word(GRID + GRID_WALKED_AT) + WALKED_LIST + drawn;
}
