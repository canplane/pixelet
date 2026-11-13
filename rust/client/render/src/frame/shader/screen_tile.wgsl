// pxl-render/frame/shader/screen_tile.wgsl

// How a pass over the whole screen or the whole map takes its tiles so that it touches only
// the damaged ones (gpu/tile.rs `TileGrid`). A draw takes a quad a tile, drawn instanced, and
// collapses the quad of an undamaged tile. A compute pass takes a thread a tile, which returns
// at once on an undamaged one. A tile is damaged where it changed this frame or has not yet
// run through the jitter's sequence since, and every tile is damaged where the frame is drawn
// whole. `TILE_JITTERS`, the frames in which the sequence covers a pixel, comes from word.rs
// `screen::JITTERS`.

// The uniform `TileGrid` and the rule a tile's word is read by, `tiles_age` and
// `tiles_in_damage`, come from tile_word.wgsl, which spells word.rs `screen`'s rule.
// `cnt` holds the tiles across and down, the frames a tile stays damaged once it changed
// (`damage_frames`), and this frame's number. In a frame drawn whole, `damage_frames` is
// `u32::MAX`, so every tile is damaged. `size` holds the target's pixels across and down, how
// many pixels a quad is grown each way past its tile, and a tile's side in pixels.

// Each tile's word holds the number of the frame it last changed in (gpu/tile.rs
// `TileGrid::damage`).
@group(0) @binding(30) var<storage, read> changed_at: array<u32>;
@group(0) @binding(31) var<uniform> tiles: TileGrid;
// The tiles the frame's marks cover, as the first tile across and down and the tile just past
// the last, which the GPU leaves once the frame's marks are made (gpu/tile.rs
// `TileGrid::new_damage_rect`, tile_mark.wgsl).
@group(0) @binding(33) var<storage, read> new_damage_rect: vec4u;

// A tile's side, in pixels.
fn tile_side() -> u32 {
  return u32(tiles.size.w);
}

// How many frames have passed since tile `ii` last changed; 0 means it changed this frame.
fn tile_age(ii: u32) -> u32 {
  return tiles_age(tiles, changed_at[ii]);
}

// Whether tile `ii` is damaged.
fn tile_in_damage(ii: u32) -> bool {
  return tiles_in_damage(tiles, changed_at[ii]);
}

fn tile_corner(vi: u32) -> vec2u {
  let corners = array<vec2u, 6>(
    vec2u(0u, 0u), vec2u(1u, 0u), vec2u(0u, 1u),
    vec2u(0u, 1u), vec2u(1u, 0u), vec2u(1u, 1u),
  );
  return corners[vi];
}

fn tile_px_clip(px: vec2f) -> vec4f {
  let size = tiles.size.xy;
  return vec4f(px.x / size.x * 2.0 - 1.0, 1.0 - px.y / size.y * 2.0, 0.0, 1.0);
}

// Corner `vi` of tile `ii`'s quad in clip space, grown past the tile by `size.z` pixels. Every
// corner of an undamaged tile lies outside the clip volume, so its quad draws nothing.
fn tile_clip(vi: u32, ii: u32) -> vec4f {
  let t = vec2u(ii % tiles.cnt.x, ii / tiles.cnt.x);
  let c = tile_corner(vi);
  let px = min(vec2f((t + c) * tile_side()), tiles.size.xy) + (vec2f(c) * 2.0 - 1.0) * tiles.size.z;
  return select(vec4f(2.0, 2.0, 0.0, 1.0), tile_px_clip(px), tile_in_damage(ii));
}

// Corner `vi` of the quad of tile `ii` of the damage rectangle, counted row by row, in clip
// space; an undamaged tile's quad draws nothing. The draw has one instance a tile of the
// rectangle (gpu/tile.rs `TileGrid::rect_args`). It draws exactly the damaged tiles only where
// no tile outside the rectangle is damaged, which holds for a target whose tiles stay damaged
// only in the frame they changed.
fn rect_clip(vi: u32, ii: u32) -> vec4f {
  let b = new_damage_rect;
  let w = b.z - b.x;
  let t = b.xy + vec2u(ii % w, ii / w);
  return tile_clip(vi, t.y * tiles.cnt.x + t.x);
}
