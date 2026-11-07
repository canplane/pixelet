// pxl-render/frame/shader/claimed_box.wgsl

// The box of the screen's tiles claimed this frame, a thread a tile, once the frame's marks
// are made (gpu/tiles.rs `Tiles::dispatch_claimed_box`): its first tile across and down and
// its last, none where the first lies past the last, as the box is set before the pass.

@group(0) @binding(0) var<storage, read_write> claimed_box: array<atomic<u32>, 4>;
@group(0) @binding(30) var<storage, read> changed_at: array<u32>;
@group(0) @binding(31) var<uniform> tiles: Tiles;

@compute @workgroup_size(64, 1, 1)
fn claimed_box_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let across = tiles.count.x;
  if (id.x >= across * tiles.count.y || !tiles_claimed(tiles, changed_at[id.x])) {
    return;
  }
  let t = vec2<u32>(id.x % across, id.x / across);
  atomicMin(&claimed_box[0], t.x);
  atomicMin(&claimed_box[1], t.y);
  atomicMax(&claimed_box[2], t.x);
  atomicMax(&claimed_box[3], t.y);
}
