// pxl-render/frame/shader/tile_word.wgsl

// The tiles' uniform as every pass reads it (word.rs `screen::TILES_WORDS`, gpu/tile.rs
// `TileGrid::words`), and the rule a tile's word is read by, word.rs `screen::age` and
// `screen::in_damage`: a tile's age is the frames since it changed, which wraps, and a tile is
// damaged while its age is below the uniform's `damage_frame_cnt`. Every pass that reads the tiles
// (screen_tile.wgsl, screen_propagate.wgsl, cast_tile.wgsl) uses this one rule, and build.rs
// holds the Rust's rule to it.

struct TileGrid {
  cnt: vec4u,
  size: vec4f,
}

fn tiles_age(t: TileGrid, changed_at: u32) -> u32 {
  return t.cnt.w - changed_at;
}

fn tiles_in_damage(t: TileGrid, changed_at: u32) -> bool {
  return tiles_age(t, changed_at) < t.cnt.z;
}
