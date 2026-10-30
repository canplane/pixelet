// pxl-render/frame/shader/screen_tiles.wgsl

// The tiles as a pass over the whole screen or map draws them (gpu/tiles.rs
// `Resolve::mark`): a quad each, drawn instanced, an unclaimed one collapsed, so the
// pass touches the claimed tiles alone; or a thread each, an unclaimed one
// returning at once. A tile is claimed where it changed
// this frame or has not yet run through the jitter's sequence since, or every
// one where the frame is drawn whole.

struct Tiles {
  // The tiles across and down, the frames a tile stays claimed once it
  // changed (`claimed_for`, all of them where every one is drawn), and this
  // frame's number.
  count: vec4u,
  // The target's pixels across and down, how far, in them, a quad is grown
  // each way past its tile, and a tile's side, in them.
  size: vec4f,
};

// Each tile's word, the number of the frame it last changed in
// (gpu/tiles.rs `Tiles::mark`).
@group(0) @binding(30) var<storage, read> changed_at: array<u32>;
@group(0) @binding(31) var<uniform> tiles: Tiles;
// The frame's marks, each its tiles from the first across and down to past
// the last, one of every tile where the target is drawn whole (`Tiles::marks`).
@group(0) @binding(32) var<storage, read> marks: array<vec4u>;

// The frames the jitter's sequence covers a pixel in (pxl-spec-core
// `screen::JITTERS`).
const TILE_JITTERS: u32 = 8u;

// A tile's pixels a side.
fn tile_side() -> u32 {
  return u32(tiles.size.w);
}

// Tile `ii`'s frames since it last changed: 0 where it changed this frame.
fn tile_age(ii: u32) -> u32 {
  return tiles.count.w - changed_at[ii];
}

// Whether tile `ii` is claimed.
fn tile_claimed(ii: u32) -> bool {
  return tile_age(ii) < tiles.count.z;
}

// A quad's corner `vi`, two triangles.
fn tile_corner(vi: u32) -> vec2u {
  let corners = array<vec2u, 6>(
    vec2u(0u, 0u), vec2u(1u, 0u), vec2u(0u, 1u),
    vec2u(0u, 1u), vec2u(1u, 0u), vec2u(1u, 1u),
  );
  return corners[vi];
}

// The pixel `px` of the target in clip units.
fn tile_px_clip(px: vec2f) -> vec4f {
  let size = tiles.size.xy;
  return vec4f(px.x / size.x * 2.0 - 1.0, 1.0 - px.y / size.y * 2.0, 0.0, 1.0);
}

// Corner `vi` of tile `ii`'s quad, in clip units; past the clip where the
// tile is not claimed.
fn tile_clip(vi: u32, ii: u32) -> vec4f {
  let t = vec2u(ii % tiles.count.x, ii / tiles.count.x);
  let c = tile_corner(vi);
  let px = min(vec2f((t + c) * tile_side()), tiles.size.xy) + (vec2f(c) * 2.0 - 1.0) * tiles.size.z;
  return select(vec4f(2.0, 2.0, 0.0, 1.0), tile_px_clip(px), tile_claimed(ii));
}

// Corner `vi` of mark `ii`'s quad, over its tiles, in clip units: where the
// tiles claimed are exactly the frame's marks', as the light's map's are.
fn mark_clip(vi: u32, ii: u32) -> vec4f {
  let m = marks[ii];
  let c = tile_corner(vi);
  let t = select(m.xy, m.zw, c == vec2u(1u));
  return tile_px_clip(min(vec2f(t * tile_side()), tiles.size.xy));
}
