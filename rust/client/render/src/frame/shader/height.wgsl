// pxl-render/height.wgsl

// The columns' ground tops of the bricks `fresh` names, the bricks handed their tiles'
// samples this turn, one a `z`, each a record as handed (`clipmap::HANDED_WORDS`): a
// thread a column (pxl-engine fill.rs `heights`), off its tile's samples
// (ground_height.wgsl), an even column's laying its own palette index and the next three's
// as one word; and its cell's four palette indices into their own buffer
// (`fill::put_palette_word`), which the cast draws by.

@group(0) @binding(0) var<storage, read> fresh: array<u32>;
@group(0) @binding(1) var<storage, read_write> ground: array<u32>;
@group(0) @binding(2) var<storage, read_write> palette_indices: array<u32>;

/// A cell's own look at `cell`, the majority of the four it folds, the tie corner turning
/// with the zoom (`ground::own`, `ground::majority`).
fn own(w: u32, cell: u32) -> u32 {
  let v = palette_four(w);
  let tie = select(v.w, v.x, (cell & 1u) == 1u);
  var half = 0u;
  var halved = false;
  for (var k = 0u; k < 4u; k += 1u) {
    let x = v[k];
    let n = dot(select(vec4<u32>(0u), vec4<u32>(1u), v == vec4<u32>(x)), vec4<u32>(1u));
    if (2u * n > 4u) {
      return x;
    }
    if (2u * n == 4u) {
      if (halved && half != x) {
        return tie;
      }
      half = x;
      halved = true;
    }
  }
  return select(tie, half, halved);
}

@compute @workgroup_size(HEIGHT_SIDE, HEIGHT_SIDE, 1)
fn height_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x >= BRICK || id.y >= BRICK) {
    return;
  }
  let rec = handed_at(id.z);
  let slot = fresh[rec + FILL_BRICK];
  let base = brick_at(slot);
  let place_word = fresh[rec + FILL_PLACE];
  let place = vec2<u32>(place_word & 0xffffu, place_word >> 16u);
  let cell = fresh[rec + FILL_CELL];
  let i = cell_id(id.xy);
  let four = fresh[rec + PALETTE_AT + i];
  var top = SEA_LEVEL;
  if (own(four, cell) != VALUE_UNKNOWN) {
    top = ground_height(rec, vec2<i32>(place * BRICK + id.xy));
  }
  ground[base + COLUMNS_AT + i] = u32(top) & 0xffffu;
  palette_indices[slot * BRICK_CELLS + i] = four;
  if ((id.x | id.y) % 2u == 1u) {
    return;
  }
  var word = 0u;
  for (var j = 0u; j < LAID_CELLS; j += 1u) {
    word |= own(fresh[rec + PALETTE_AT + i + j], cell) << (8u * j);
  }
  ground[base + LAID_AT + i / LAID_CELLS] = word;
}
