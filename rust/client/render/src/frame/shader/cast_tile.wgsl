// pxl-render/frame/shader/cast_tile.wgsl

// Lists the tiles claimed this frame for the pixels' cast (cast.wgsl `cast_cs`), one thread a
// tile, ahead of the cast in its pass. The list's count is the first word of the cast's indirect
// dispatch, so the cast takes the claimed tiles' pixels alone. A frame drawn whole lists every
// tile.

struct TileList {
  // The dispatch's workgroups along x, one for each tile listed, up to as many as the list
  // holds.
  count: atomic<u32>,
  // A tile's workgroups across and down, the dispatch's y and z. The frame writes them when it
  // sets the count to 0 (ray/cast.rs `CAST_TILES_START`).
  sides: array<u32, 2>,
  // How many tiles the threads have listed, counted as each takes its place.
  cnt: atomic<u32>,
  // The tiles listed, each with its row in the high half of its word and its column in the
  // low half.
  at: array<u32>,
}

@group(0) @binding(0) var<storage, read_write> list: TileList;
@group(0) @binding(30) var<storage, read> changed_at: array<u32>;
@group(0) @binding(31) var<uniform> tiles: TileGrid;

@compute @workgroup_size(CAST_TILES_LANES, 1, 1)
fn cast_tiles_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let across = tiles.cnt.x;
  // The list holds a place for each tile.
  let cap = across * tiles.cnt.y;
  if (id.x >= cap || !tiles_in_damage(tiles, changed_at[id.x])) {
    return;
  }
  let place = atomicAdd(&list.cnt, 1u);
  if (place >= cap) {
    return;
  }
  atomicMax(&list.count, workgroups(place + 1u, cap, 1u));
  list.at[place] = ((id.x / across) << 16u) | (id.x % across);
}
