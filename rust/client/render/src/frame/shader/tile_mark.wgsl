// pxl-render/frame/shader/tile_mark.wgsl

// A changed draw's mark, its own fragments (gpu/pass/skinned.rs `draw_marks`):
// drawn where it was and where it is, its vertices as its depth pass lays
// them, each fragment sets its screen tile's word to this frame's number, as
// a mark the host hands sets it (screen_mark.wgsl). No color, the world's
// depth attached read only and not tested: what the draw covers, seen or not.

struct TileMarks {
  // The tiles across and down, the frames a tile stays claimed, this frame's
  // number; then the target's size, a quad's growth and a tile's side.
  count: vec4u,
  size: vec4f,
};

@group(0) @binding(30) var<storage, read_write> changed_at: array<u32>;
@group(0) @binding(31) var<uniform> tile_marks: TileMarks;

@fragment
fn fs(@builtin(position) clip: vec4f) {
  let t = vec2u(clip.xy) / u32(tile_marks.size.w);
  changed_at[t.y * tile_marks.count.x + t.x] = tile_marks.count.w;
}
