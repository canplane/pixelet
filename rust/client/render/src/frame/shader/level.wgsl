// pxl-render/level.wgsl

// A brick's levels as they lie in its words, each level where world_word.wgsl's `level_at`
// says it begins (pxl-engine clipmap.rs `level_at`), its entries in clip.wgsl's cell order;
// the bounds an entry holds are world_rule.wgsl's. It reads no binding.

/// The first word of entry `at` of level `j` of the brick at `base` (`fill::level_of`).
fn level_entry(base: u32, j: u32, at: vec2<u32>) -> u32 {
  return base + LEVELS_AT + LEVEL_ENTRY_WORDS * (level_at(j) + cell_id(at));
}
