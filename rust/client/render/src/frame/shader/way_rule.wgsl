// pxl-render/way_rule.wgsl

// The ways' rule that reads no buffer, by hand (pxl-engine way.rs): which cells a run of
// leaf cells fills, and a column's face where it has none. What the spans' pass (way.wgsl)
// and the tree's (way_tree.wgsl) both read.

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
