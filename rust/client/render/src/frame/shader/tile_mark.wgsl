// pxl-render/frame/shader/tile_mark.wgsl

// Marks the tiles a changed draw covers, from the draw's own fragments (gpu/pass/mark.rs
// `Marks`). The draw is drawn where it was and where it is, each time with the frame's words
// it was or is drawn with (group 0's `frame_words`), its jitter among them, so its fragments
// are the pixels it covered then and covers now. Its vertices are placed as its depth pass
// places them. Each fragment sets its tile's word to this frame's number, as a mark the host
// hands sets it (screen_mark.wgsl). The one fragment whose exchange set the word widens the
// rectangle of tiles the frame's marks cover to hold the tile, so each tile is marked once,
// however many of its fragments are in flight. The pass writes no color, and the target's depth
// is attached read-only and not tested, so the mark covers what the draw covers, seen or not.

struct TileMarks {
  // Laid out as tile_word.wgsl's `Tiles`: `count` holds the tiles across and down, the frames a
  // tile stays claimed and this frame's number, and `size` the target's size, a quad's growth
  // and a tile's side.
  count: vec4u,
  size: vec4f,
};

@group(0) @binding(30) var<storage, read_write> changed_at: array<atomic<u32>>;
@group(0) @binding(31) var<uniform> tile_marks: TileMarks;
// The tiles the frame's marks cover, as the first tile across and down and the tile just past
// the last (gpu/tile.rs `Tiles::new_damage_rect`).
@group(0) @binding(32) var<storage, read_write> new_damage_rect: array<atomic<u32>, 4>;

@fragment
fn fs(@builtin(position) clip: vec4f) {
  let t = vec2u(clip.xy) / u32(tile_marks.size.w);
  let ii = t.y * tile_marks.count.x + t.x;
  let now = tile_marks.count.w;
  // The word is read first, so a tile already set this frame takes no write.
  if (atomicLoad(&changed_at[ii]) == now || atomicExchange(&changed_at[ii], now) == now) {
    return;
  }
  atomicMin(&new_damage_rect[0], t.x);
  atomicMin(&new_damage_rect[1], t.y);
  atomicMax(&new_damage_rect[2], t.x + 1u);
  atomicMax(&new_damage_rect[3], t.y + 1u);
}
