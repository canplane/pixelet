// pxl-render/frame/shader/box_args.wgsl

// The draw over the box of the tiles a frame's marks cover (gpu/tiles.rs
// `Tiles::dispatch_box_args`), once the marks are made: the indirect args of
// a quad a tile of the box (screen_tiles.wgsl `box_clip`), none where the box
// holds none, and no more than the tiles the target holds.

@group(0) @binding(0) var<storage, read> marks_box: vec4u;
@group(0) @binding(1) var<storage, read_write> box_args: array<u32, 4>;
@group(0) @binding(2) var<uniform> tiles: Tiles;

@compute @workgroup_size(1, 1, 1)
fn box_args_cs() {
  let b = marks_box;
  let n = select(0u, (b.z - b.x) * (b.w - b.y), b.z > b.x && b.w > b.y);
  box_args[0] = 6u;
  box_args[1] = in_room(n, tiles.count.x * tiles.count.y);
  box_args[2] = 0u;
  box_args[3] = 0u;
}
