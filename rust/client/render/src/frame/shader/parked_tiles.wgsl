// pxl-render/parked_tiles.wgsl

// The screen's tiles that hold a pixel the eye's holes' queue left parked, each set changed
// in this frame (gpu/tiles.rs), a thread a hole the queue took (walked.wgsl; queue_last.wgsl
// `unreached`), after the queue. A frame through the gate claims a tile while it changed
// fewer than `screen::CLAIMED_FOR` frames ago, so the frames that follow take a parked
// pixel's tile into their cast and their draw whatever else they claim (cast_tiles.wgsl), and
// no pixel is left parked where the gate would not take it again.

@group(0) @binding(3) var rays: texture_storage_2d_array<r32uint, read_write>;
@group(0) @binding(8) var<storage, read> beams: array<u32>;
@group(0) @binding(30) var<storage, read_write> changed_at: array<u32>;
@group(0) @binding(31) var<uniform> tiles: Tiles;

@compute @workgroup_size(CAST_TILES_LANES, 1, 1)
fn parked_tiles_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let size = textureDimensions(rays).xy;
  let n = min(beams[walked_cnt_at()], size.x * size.y);
  if (id.x >= n) {
    return;
  }
  let listed = beams[walked_list_at(id.x)];
  let p = vec2<u32>(listed & 0xffffu, listed >> 16u);
  let at = eye_state_at(p, size, arrayLength(&beams)) + STATE_WORDS - 1u;
  if (any(p >= size) || at >= arrayLength(&beams) || (beams[at] & PARKED) == 0u) {
    return;
  }
  let t = p / SCREEN_TILE;
  let k = t.y * tiles.count.x + t.x;
  if (k < arrayLength(&changed_at)) {
    changed_at[k] = tiles.count.w;
  }
}
