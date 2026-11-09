// pxl-render/frame/shader/screen_mark.wgsl

// Sets the frame's mark into the tiles' words (gpu/tile.rs `TileGrid::damage`), for the screen's
// tiles. A frame hands one mark, a rectangle of tiles, and a frame drawn
// whole hands a mark of every tile. One thread a tile of the mark writes this frame's number
// into the tile's word in `changed_at`, as a light tile's word takes the number of the cast
// that lists it (light_receiver_tile.wgsl).

struct Damage {
  // `xy` is the mark's first tile across and down, and `zw` the tile just past its last.
  tiles: vec4u,
  // `x` is this frame's number and `y` the number of tiles across; `zw` are unused.
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
