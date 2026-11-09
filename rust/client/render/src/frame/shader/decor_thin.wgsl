// pxl-render/decor_thin.wgsl

// Lays the candidates of each pair that `decor_mark_cs` listed whole, one thread a clip level
// cell of the pair's brick. Each leaf cell under that cell is read from the decor's hashes. A
// candidate that stands at the entry's clip level and stands clear (`scatter::retained`) is
// laid as the decor's next record, which holds its cell for `decor_pose_cs`. The brick is the one the entry's header names, which the mark wrote from
// the clipmap's row this frame. The row is not read again, so a header that names another brick
// lays that brick's candidates. The pass reads no world, only the decor's hashes.

/// Decor's records (pxl-content `generation::ENTRY_RECS_AT`). The entries' headers are atomic, as
/// each decor's count, which this pass's threads add to, lies among them. The records after
/// them are plain: a record's words are written by the one thread whose add to its decor's
/// count took it, and read by the passes after.
struct RecordBuffer {
  headers: array<atomic<u32>, ENTRY_RECS_AT>,
  words: array<u32>,
}

@group(0) @binding(9) var<storage, read_write> recs: RecordBuffer;
@group(0) @binding(11) var<storage, read> list: array<u32>;

@compute @workgroup_size(LANES)
fn decor_thin_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let pair = list[LIST_AT + id.z];
  let brick = pair_entry(pair);
  let j = pair_decor(pair);
  let cell_zoom = FIRST_CELL_ZOOM + brick / CLIP_ENTRIES;
  let header = brick * HEADER_WORDS;
  let q = vec2<i32>(i32(atomicLoad(&recs.headers[header])), i32(atomicLoad(&recs.headers[header + 1u])));
  let s = LEAF_CELL_ZOOM - cell_zoom;
  let corner = (q << vec2<u32>(BRICK_SHIFT + s))
    + vec2<i32>(i32(id.x % BRICK), i32(id.x / BRICK)) * (1i << s);
  if (scatter_cnt(j, cell_zoom, corner >> vec2<u32>(s)) == 0u) {
    return;
  }
  let side = 1u << s;
  for (var n = 0u; n < side * side; n += 1u) {
    let leaf = corner + vec2<i32>(i32(n % side), i32(n / side));
    if (scatter_cnt(j, LEAF_CELL_ZOOM, leaf) == 0u) {
      continue;
    }
    let c = candidate_at(j, leaf);
    if (stands_at(j, variant_of(j, c.seed), cell_zoom) && retained(j, c)) {
      let k = atomicAdd(&recs.headers[cnt_at(brick, j)], 1u);
      let r = entry_word(brick, j, k) - ENTRY_RECS_AT;
      recs.words[r + 1u] = u32(leaf.x);
      recs.words[r + 2u] = u32(leaf.y);
    }
  }
}
