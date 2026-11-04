// pxl-render/level.wgsl

// A brick's levels as they lie in its words (pxl-engine clipmap.rs `level_at`), over
// brick.wgsl and clip.wgsl's cell order; the bounds an entry holds are world_rule.wgsl's.
// It reads no binding.

/// The first word of entry `at` of level `j` of the brick at `base` (`fill::level_of`).
fn level_word(base: u32, j: u32, at: vec2<u32>) -> u32 {
  let first = (4u * BRICK_CELLS - ((4u * BRICK_CELLS) >> (2u * j))) / 3u;
  return base + LEVELS_AT + LEVEL_ENTRY_WORDS * (first + cell_id(at));
}
