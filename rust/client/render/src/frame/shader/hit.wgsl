// pxl-render/hit.wgsl

// The missed pixels' hits the walks again keep among the beam rays' words (pxl-engine
// `beam::HIT_WORDS`), `HITS` at most.

/// The first word of the hit numbered `r` (`Grid::hit_at`).
fn hit_at(r: u32) -> u32 {
  return eye_word(GRID + GRID_HITS_AT) + r * HIT_WORDS;
}
