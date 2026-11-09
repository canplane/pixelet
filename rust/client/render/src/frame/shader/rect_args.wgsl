// pxl-render/frame/shader/rect_args.wgsl

// Writes the indirect arguments of the draw over the rectangle of tiles the frame's damage
// covers, once every pass that marks has made that rectangle (gpu/tile.rs
// `TileGrid::dispatch_rect_args`). The draw is one quad a tile of the rectangle (screen_tile.wgsl
// `rect_clip`): none where the rectangle is empty, and never more than the target's tiles.

@group(0) @binding(0) var<storage, read> new_damage_rect: vec4u;
@group(0) @binding(1) var<storage, read_write> rect_args: array<u32, 4>;
@group(0) @binding(2) var<uniform> tiles: TileGrid;

@compute @workgroup_size(1, 1, 1)
fn rect_args_cs() {
  let b = new_damage_rect;
  let n = select(0u, (b.z - b.x) * (b.w - b.y), b.z > b.x && b.w > b.y);
  rect_args[0] = 6u;
  rect_args[1] = in_cap(n, tiles.cnt.x * tiles.cnt.y);
  rect_args[2] = 0u;
  rect_args[3] = 0u;
}
