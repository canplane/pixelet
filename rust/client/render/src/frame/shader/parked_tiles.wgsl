// pxl-render/parked_tiles.wgsl

// What the eye's holes' queue left owed, after the queue: the holes of its owed bits, a thread a
// word, and the rays it left parked in its run's pool, a thread a place (walked.wgsl; queue.wgsl
// `unreached`). Each is counted as the eye's owed work (pxl-engine `node_cache::OWED_AT`), which
// the renderer reads back and wakes the next frame by, and its screen tile set changed in this
// frame (gpu/tiles.rs). A frame through the gate claims a tile while it changed fewer than
// `screen::CLAIMED_FOR` frames ago, so the frames that follow take an owed pixel's tile into
// their cast and their draw whatever else they claim (cast_tiles.wgsl), and no pixel is left
// owed where the gate would not take it again.

@group(0) @binding(3) var rays: texture_storage_2d_array<r32uint, read_write>;
@group(0) @binding(8) var<storage, read> beams: array<u32>;
@group(0) @binding(9) var<storage, read_write> node_cache: array<atomic<u32>>;
@group(0) @binding(30) var<storage, read_write> changed_at: array<u32>;
@group(0) @binding(31) var<uniform> tiles: Tiles;

fn word_of(k: u32) -> u32 {
  return select(0u, beams[k], k < arrayLength(&beams));
}

/// Pixel `p`'s tile set changed, and one more owed counted.
fn owed_at(p: vec2<u32>) {
  atomicAdd(&node_cache[EYE_OWED], 1u);
  let t = p / SCREEN_TILE;
  let k = t.y * tiles.count.x + t.x;
  if (k < arrayLength(&changed_at)) {
    changed_at[k] = tiles.count.w;
  }
}

/// A thread's place: its workgroup's, `PARKED_TILES_ACROSS` a row, times the lanes, and its
/// lane.
@compute @workgroup_size(CAST_TILES_LANES, 1, 1)
fn parked_tiles_cs(@builtin(workgroup_id) group: vec3<u32>, @builtin(local_invocation_id) lane: vec3<u32>) {
  let k = (group.y * PARKED_TILES_ACROSS + group.x) * CAST_TILES_LANES + lane.x;
  let size = textureDimensions(rays).xy;
  let words = hole_words(size);
  if (k < words) {
    // A word's pixels lie in one square, which lies in one tile.
    if (word_of(owed_bits_at(size) + k) != 0u) {
      owed_at(bit_px(k, 0u, size));
    }
    return;
  }
  let place = k - words;
  let pool = pool_at(size, owed_into(word_of(from_at(size))));
  if (place >= min(word_of(pool + POOL_CNT), PARKED_ROOM)) {
    return;
  }
  let at = pool + POOL_STATES + place * PARK_WORDS;
  if ((word_of(at + 4u) & PARKED) != 0u) {
    let px = word_of(at + PARK_PX);
    owed_at(vec2<u32>(px & 0xffffu, px >> 16u));
  }
}
