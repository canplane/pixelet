// pxl-render/brick.wgsl

// A brick as it lies in its words (pxl-engine clipmap.rs `BRICK_WORDS`, `column_top`), its
// cells in clip.wgsl's order. It reads no binding, so a pass that writes the bricks takes it
// as one that reads them does.

/// The first of the words of the brick held in `slot`.
fn brick_at(slot: u32) -> u32 {
  return slot * BRICK_WORDS;
}

/// A column word's ground top (`clipmap::column_top`).
fn column_top(w: u32) -> i32 {
  return i32(w << 16u) >> 16u;
}
