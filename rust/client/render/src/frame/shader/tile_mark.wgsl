// pxl-render/frame/shader/tile_mark.wgsl

// A changed draw's mark, its own fragments (gpu/pass/mark.rs `Marks`): drawn
// where it was and where it is, its vertices as its depth pass lays them,
// each fragment sets its tile's word to this frame's number, as a mark the
// host hands sets it (screen_mark.wgsl), and widens the box of the tiles the
// frame's marks cover to hold it, the first fragment to set a tile alone. No
// color, the target's depth attached read only and not tested: what the draw
// covers, seen or not.

struct TileMarks {
  // The tiles across and down, the frames a tile stays claimed, this frame's
  // number; then the target's size, a quad's growth and a tile's side.
  count: vec4u,
  size: vec4f,
};

@group(0) @binding(30) var<storage, read_write> changed_at: array<u32>;
@group(0) @binding(31) var<uniform> tile_marks: TileMarks;
// The tiles the frame's marks cover, from the first across and down to past
// the last (gpu/tiles.rs `Tiles::marks_box`).
@group(0) @binding(32) var<storage, read_write> marks_box: array<atomic<u32>, 4>;

@fragment
fn fs(@builtin(position) clip: vec4f) {
  let t = vec2u(clip.xy) / u32(tile_marks.size.w);
  let ii = t.y * tile_marks.count.x + t.x;
  if (changed_at[ii] == tile_marks.count.w) {
    return;
  }
  changed_at[ii] = tile_marks.count.w;
  atomicMin(&marks_box[0], t.x);
  atomicMin(&marks_box[1], t.y);
  atomicMax(&marks_box[2], t.x + 1u);
  atomicMax(&marks_box[3], t.y + 1u);
}
