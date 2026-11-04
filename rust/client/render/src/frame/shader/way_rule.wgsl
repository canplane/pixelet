// pxl-render/way_rule.wgsl

// The ways' rule that reads no buffer, by hand (pxl-engine way.rs): which cells a run of
// leaf cells fills, a cell's six about it, a column's face where it has none, and where a
// column's spans lie in a brick's scratch (fill.rs `Scratch`). What the spans' pass
// (way.wgsl), the tree's (way_tree.wgsl) and the levels' (bound.wgsl, spans.wgsl) read.

/// `i32::MIN`, a column's face where it has none (`way::Spans::NONE`).
const FACE_NONE: i32 = -2147483647i - 1i;

/// Whether the leaf cells `run`, `[lo, hi)`, fill cell `z` of the grid's zoom `cell`
/// (`way::fills`): more than half its height, half a cell or thinner filling none.
fn fills(cell: u32, run: vec2<i32>, z: i32) -> bool {
  if (cell >= LEAF_CELL) {
    let leaf = z >> (cell - LEAF_CELL);
    return run.x <= leaf && leaf < run.y;
  }
  let s = 1i << (LEAF_CELL - cell);
  return 2i * (min(run.y, (z + 1i) * s) - max(run.x, z * s)) > s;
}

/// The cells of zoom `cell` the leaf cells `run` fill, first and last, first past last
/// where none (`way::fill_range`).
fn fill_range(cell: u32, run: vec2<i32>) -> vec2<i32> {
  if (run.x >= run.y) {
    return vec2<i32>(1i, 0i);
  }
  if (cell >= LEAF_CELL) {
    let d = cell - LEAF_CELL;
    return vec2<i32>(run.x << d, (run.y << d) - 1i);
  }
  let d = LEAF_CELL - cell;
  let first = run.x >> d;
  let last = (run.y - 1i) >> d;
  return vec2<i32>(select(first + 1i, first, fills(cell, run, first)), select(last - 1i, last, fills(cell, run, last)));
}

/// The `k`-th of a cell and the six about it, from its own: the row under it, the column
/// before it, the cell under it, the cell over it, the column after it, the row over it
/// (`way::near`).
fn near(k: u32) -> vec3<i32> {
  var i = 6u - k;
  var s = 1i;
  if (k < 4u) {
    i = k - 1u;
    s = -1i;
  }
  let on = (vec3<u32>(1u, 0u, 2u) == vec3<u32>(i)) & vec3<bool>(k != 0u);
  return select(vec3<i32>(0i), vec3<i32>(s), on);
}

/// A head word's face cell (fill.rs `Scratch::head`).
fn head_face(w: u32) -> i32 {
  let face = i32(w) >> 16u;
  return select(face, FACE_NONE, face == -32768i);
}

/// Where column `c`'s spans begin in the scratch from `mine`, `c` in the brick's cells,
/// its ring one past each side (fill.rs `Scratch::column`).
fn spans_at(mine: u32, c: vec2<i32>) -> u32 {
  let i = u32(c.y + 1i) * SPANS_ACROSS + u32(c.x + 1i);
  return mine + SPANS_AT + i * SPANS_COLUMN_WORDS;
}
