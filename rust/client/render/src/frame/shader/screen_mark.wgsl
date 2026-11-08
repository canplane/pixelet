// pxl-render/frame/shader/screen_mark.wgsl

// The tiles marked (gpu/tiles.rs `Tiles::damage`), the screen's or the light's
// map's, as a light tile's word is marked with the cast that lists it
// (light_receiver_tiles.wgsl): a thread a tile of the frame's one
// mark, whose word (`changed_at`) takes this frame's number; a frame drawn
// whole hands one mark of every tile.

struct Damage {
  // The mark's tiles, from its first across and down to past its last.
  tiles: vec4u,
  // This frame's number, the tiles across, two unused.
  header: vec4u,
};

@group(0) @binding(0) var<uniform> damage: Damage;
@group(0) @binding(1) var<storage, read_write> changed_at: array<u32>;

@compute @workgroup_size(64, 1, 1)
fn screen_mark_cs(@builtin(global_invocation_id) id: vec3u) {
  let m = damage.tiles;
  let w = m.z - m.x;
  if (id.x >= w * (m.w - m.y)) {
    return;
  }
  let t = m.xy + vec2u(id.x % w, id.x / w);
  changed_at[t.y * damage.header.y + t.x] = damage.header.x;
}
