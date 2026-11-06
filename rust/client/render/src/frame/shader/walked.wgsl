// pxl-render/walked.wgsl

// Where the eye's walked pixels' lists lie among the beam rays' words, past the camera grid's
// (pxl-engine `beam::Grid::walked_at`, `ray::walked_words`), room for every pixel drawn: the
// pixels the reading walk marked, after their count (cast.wgsl `cast_cs`), which the holes'
// queue takes (queue.wgsl).

/// The word of the marked pixels' count (`ray::WALKED_CNT`).
fn walked_cnt_at() -> u32 {
  return eye_word(GRID + GRID_WALKED_AT) + WALKED_CNT;
}

/// The word of the marked pixel at place `k` (`ray::WALKED_LIST`).
fn walked_list_at(k: u32) -> u32 {
  return eye_word(GRID + GRID_WALKED_AT) + WALKED_LIST + k;
}
