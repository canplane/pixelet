// pxl-render/brick.wgsl

// Where a brick lies in its words (pxl-engine clipmap.rs `BRICK_WORDS`, `column_top`), with
// its cells in clip.wgsl's order. The part reads no binding, so a pass that writes the bricks
// includes it as a pass that reads them does.

fn brick_at(slot: u32) -> u32 {
  return slot * BRICK_WORDS;
}

/// The ground's top in a column word, a signed 16-bit value in its low half
/// (`clipmap::column_top`).
fn column_top(w: u32) -> i32 {
  return i32(w << 16u) >> 16u;
}
