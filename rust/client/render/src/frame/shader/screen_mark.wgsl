// pxl-render/frame/shader/screen_mark.wgsl

// The tiles marked (gpu/tiles.rs `Tiles::mark`), the screen's or the light's
// map's, as a light tile's word is marked with the cast that reads it
// (pxl-shader light.rs `light_mark_cs`): a workgroup row a mark, a thread a
// tile of its tiles, whose word (`changed_at`) takes this frame's number; a
// frame drawn whole hands one mark of every tile.

struct Mark {
  // How many marks, this frame's number, the tiles across, one unused.
  head: vec4u,
};

@group(0) @binding(0) var<uniform> mark: Mark;
// Each mark's tiles, from its first across and down to past its last.
@group(0) @binding(1) var<storage, read> marks: array<vec4u>;
@group(0) @binding(2) var<storage, read_write> changed_at: array<u32>;

@compute @workgroup_size(64, 1, 1)
fn screen_mark_cs(@builtin(global_invocation_id) id: vec3u) {
  if (id.y >= mark.head.x) {
    return;
  }
  let m = marks[id.y];
  let w = m.z - m.x;
  if (id.x >= w * (m.w - m.y)) {
    return;
  }
  let t = m.xy + vec2u(id.x % w, id.x / w);
  changed_at[t.y * mark.head.z + t.x] = mark.head.y;
}
