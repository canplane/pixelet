// pxl-render/frame/shader/cast_tiles.wgsl

// The tiles claimed this frame, listed for the pixels' cast (cast.wgsl `cast_cs`), a thread a
// tile, ahead of it in its pass: the list's count is the first word of that cast's dispatch, so
// it takes a tile's pixels alone, where a frame drawn whole lists every tile.

struct CastTiles {
  // The tiles the dispatch takes: those listed, as many as the list holds.
  count: atomic<u32>,
  // A tile's workgroups across and down, set with the count's 0 (gpu/ray/cast.rs
  // `CAST_TILES_START`).
  sides: array<u32, 2>,
  // The tiles listed, counted as they are.
  cnt: atomic<u32>,
  // A tile, its row over its column.
  at: array<u32>,
}

@group(0) @binding(0) var<storage, read_write> list: CastTiles;
@group(0) @binding(30) var<storage, read> changed_at: array<u32>;
@group(0) @binding(31) var<uniform> tiles: Tiles;

@compute @workgroup_size(CAST_TILES_LANES, 1, 1)
fn cast_tiles_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let across = tiles.count.x;
  // The list holds a place for each tile.
  let room = across * tiles.count.y;
  if (id.x >= room || !tiles_claimed(tiles, changed_at[id.x])) {
    return;
  }
  let place = atomicAdd(&list.cnt, 1u);
  if (place >= room) {
    return;
  }
  atomicMax(&list.count, workgroups(place + 1u, room, 1u));
  list.at[place] = ((id.x / across) << 16u) | (id.x % across);
}
