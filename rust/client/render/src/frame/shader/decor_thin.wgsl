// pxl-render/decor_thin.wgsl

// The candidates of a pair `decor_mark_cs` listed whole, a clip level cell of its brick a
// thread: each leaf cell under it is read off the decor's hashes, and one that stands at the
// entry's clip level and stands clear (`scatter::retained`) is laid as the decor's next
// record, its cell for `decor_cs`; none past the decor's room. Its brick is its head's, which
// mark wrote off the clipmap's row this frame; the row is not read again, so a head that names
// another brick lays that brick's candidates. What it reads is the decor's hash alone.

/// Decor's records (pxl-content `generation::SLOTS_AT`): the entries' heads atomic, each
/// decor's count among them, which this pass's threads add to; then the records plain. A
/// record's words are written by the one thread whose add to its decor's count took it, and
/// read by the passes after.
struct Records {
  headers: array<atomic<u32>, SLOTS_AT>,
  words: array<u32>,
}

@group(0) @binding(9) var<storage, read_write> records: Records;
@group(0) @binding(11) var<storage, read> list: array<u32>;

@compute @workgroup_size(LANES)
fn decor_thin_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let pair = list[LIST_AT + id.z];
  let brick = pair_entry(pair);
  let j = pair_decor(pair);
  let cell_zoom = FIRST_CELL_ZOOM + brick / CLIP_ENTRIES;
  let header = brick * HEADER_WORDS;
  let q = vec2<i32>(i32(atomicLoad(&records.headers[header])), i32(atomicLoad(&records.headers[header + 1u])));
  let s = LEAF_CELL_ZOOM - cell_zoom;
  let corner = (q << vec2<u32>(BRICK_SHIFT + s))
    + vec2<i32>(i32(id.x % BRICK), i32(id.x / BRICK)) * (1i << s);
  if (scatter_count(j, cell_zoom, corner >> vec2<u32>(s)) == 0u) {
    return;
  }
  let side = 1u << s;
  for (var n = 0u; n < side * side; n += 1u) {
    let leaf = corner + vec2<i32>(i32(n % side), i32(n / side));
    if (scatter_count(j, LEAF_CELL_ZOOM, leaf) == 0u) {
      continue;
    }
    let draws = candidate_draws(j, leaf);
    if (stands_at(j, variant_of(j, draws.y), cell_zoom) && retained(j, leaf, draws.x, draws.y)) {
      let k = atomicAdd(&records.headers[cnt_at(brick, j)], 1u);
      if (k < decor_cap(j)) {
        let r = entry_word(brick, j, k) - SLOTS_AT;
        records.words[r + 1u] = u32(leaf.x);
        records.words[r + 2u] = u32(leaf.y);
      }
    }
  }
}
