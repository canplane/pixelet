// pxl-render/bound_mip.wgsl

// Where a brick's levels of bounds lie in its words. Each level begins where `bound_mip_at`
// says (world_word.wgsl, from pxl-engine clipmap.rs `bound_mip_at`), and its entries follow
// clip.wgsl's cell order. world_rule.wgsl defines the bounds an entry holds. The part reads no
// binding.

/// The first word of the entry at `at` in level `j` of the brick at `base` (pxl-engine
/// `fill::mip_of`).
fn mip_entry(base: u32, j: u32, at: vec2<u32>) -> u32 {
  return base + BOUNDS_AT + BOUND_ENTRY_WORDS * (bound_mip_at(j) + cell_id(at));
}
