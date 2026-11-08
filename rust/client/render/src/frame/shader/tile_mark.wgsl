// pxl-render/frame/shader/tile_mark.wgsl

// A changed draw's mark, its own fragments (gpu/pass/mark.rs `Marks`): drawn
// where it was and where it is, each by the frame's words (group 0's
// `frame_words`) it was or is drawn with, its jitter among them, so its
// fragments are the pixels it covered then and covers now; its vertices as
// its depth pass lays them,
// each fragment sets its tile's word to this frame's number, as a mark the
// host hands sets it (screen_mark.wgsl), and the one fragment whose exchange
// set it widens the box of the tiles the frame's marks cover to hold it: a
// tile's mark is made once, whatever its fragments in flight. No color, the
// target's depth attached read only and not tested: what the draw covers,
// seen or not.

struct TileMarks {
  // The tiles across and down, the frames a tile stays claimed, this frame's
  // number; then the target's size, a quad's growth and a tile's side.
  count: vec4u,
  size: vec4f,
};

@group(0) @binding(30) var<storage, read_write> changed_at: array<atomic<u32>>;
@group(0) @binding(31) var<uniform> tile_marks: TileMarks;
// The tiles the frame's marks cover, from the first across and down to past
// the last (gpu/tiles.rs `Tiles::new_damage_rect`).
@group(0) @binding(32) var<storage, read_write> new_damage_rect: array<atomic<u32>, 4>;

@fragment
fn fs(@builtin(position) clip: vec4f) {
  let t = vec2u(clip.xy) / u32(tile_marks.size.w);
  let ii = t.y * tile_marks.count.x + t.x;
  let now = tile_marks.count.w;
  // Read first: a tile already set this frame takes no write.
  if (atomicLoad(&changed_at[ii]) == now || atomicExchange(&changed_at[ii], now) == now) {
    return;
  }
  atomicMin(&new_damage_rect[0], t.x);
  atomicMin(&new_damage_rect[1], t.y);
  atomicMax(&new_damage_rect[2], t.x + 1u);
  atomicMax(&new_damage_rect[3], t.y + 1u);
}
