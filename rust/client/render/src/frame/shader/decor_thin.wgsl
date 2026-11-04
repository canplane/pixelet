// pxl-render/decor_thin.wgsl

// The candidates of a pair `decor_mark_cs` listed whole, a clip level cell of its brick a
// thread: each leaf cell under it is read off the decor's hashes, and one that stands at the
// entry's clip level and stands clear (`scatter::retained`) is laid as the decor's next
// record, its cell for `decor_cs`; none past the decor's room. Its brick is its head's, which
// mark wrote. What it reads is the decor's hash alone.

@group(0) @binding(9) var<storage, read_write> records: array<atomic<u32>>;
@group(0) @binding(11) var<storage, read> list: array<u32>;

@compute @workgroup_size(LANES)
fn decor_thin_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let pair = list[LIST_AT + id.z];
  let brick = pair_entry(pair);
  let j = pair_decor(pair);
  let cell = FIRST_CELL + brick / CLIP_ENTRIES;
  let head = brick * HEAD_WORDS;
  let q = vec2<i32>(i32(atomicLoad(&records[head])), i32(atomicLoad(&records[head + 1u])));
  let s = LEAF_CELL - cell;
  let corner = (q << vec2<u32>(BRICK_SHIFT + s))
    + vec2<i32>(i32(id.x % BRICK), i32(id.x / BRICK)) * (1i << s);
  if (scatter_count(j, cell, corner >> vec2<u32>(s)) == 0u) {
    return;
  }
  let side = 1u << s;
  for (var n = 0u; n < side * side; n += 1u) {
    let leaf = corner + vec2<i32>(i32(n % side), i32(n / side));
    if (scatter_count(j, LEAF_CELL, leaf) == 0u) {
      continue;
    }
    let draws = candidate_draws(j, leaf);
    if (stands_at(j, variant_of(j, draws.y), cell) && retained(j, leaf, draws.x, draws.y)) {
      let k = atomicAdd(&records[cnt_at(brick, j)], 1u);
      if (k < decor_cap(j)) {
        let r = entry_word(brick, j, k);
        atomicStore(&records[r + 1u], u32(leaf.x));
        atomicStore(&records[r + 2u], u32(leaf.y));
      }
    }
  }
}
