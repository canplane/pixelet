// pxl-render/frame/shader/rect_args.wgsl

// The draw over the box of the tiles a frame's marks cover (gpu/tiles.rs
// `Tiles::dispatch_rect_args`), once the marks are made: the indirect args of
// a quad a tile of the box (screen_tiles.wgsl `rect_clip`), none where the box
// holds none, and no more than the tiles the target holds.

@group(0) @binding(0) var<storage, read> new_damage_rect: vec4u;
@group(0) @binding(1) var<storage, read_write> rect_args: array<u32, 4>;
@group(0) @binding(2) var<uniform> tiles: Tiles;

@compute @workgroup_size(1, 1, 1)
fn rect_args_cs() {
  let b = new_damage_rect;
  let n = select(0u, (b.z - b.x) * (b.w - b.y), b.z > b.x && b.w > b.y);
  rect_args[0] = 6u;
  rect_args[1] = in_cap(n, tiles.count.x * tiles.count.y);
  rect_args[2] = 0u;
  rect_args[3] = 0u;
}
