// pxl-render/frame/shader/damage_rect.wgsl

// Finds the box of the screen's tiles claimed this frame, one thread a tile, once the frame's
// marks are made (gpu/tile.rs `TileGrid::dispatch_damage_rect`). The box holds its first tile
// across and down, then its last. A box whose first tile lies past its last holds no tile,
// which is how the box is set before the pass.

@group(0) @binding(0) var<storage, read_write> damage_rect: array<atomic<u32>, 4>;
@group(0) @binding(30) var<storage, read> changed_at: array<u32>;
@group(0) @binding(31) var<uniform> tiles: TileGrid;

@compute @workgroup_size(64, 1, 1)
fn damage_rect_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let across = tiles.count.x;
  if (id.x >= across * tiles.count.y || !tiles_in_damage(tiles, changed_at[id.x])) {
    return;
  }
  let t = vec2<u32>(id.x % across, id.x / across);
  atomicMin(&damage_rect[0], t.x);
  atomicMin(&damage_rect[1], t.y);
  atomicMax(&damage_rect[2], t.x);
  atomicMax(&damage_rect[3], t.y);
}
