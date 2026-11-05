// pxl-render/spans.wgsl

// A handed brick's columns' spans as its fill kept them in its scratch, the brick's and
// the ring of columns about it (pxl-engine fill.rs `Scratch`, `clipmap::SPANS_AT`), where
// way_rule.wgsl finds them, and the leaf cells the ways write in a column off them (way.rs
// `lined_hull`).

/// Span `j` of the column whose spans begin at `col`: its lowest and past its highest.
fn span_run(col: u32, j: u32) -> vec2<i32> {
  return bound_of(i32(scratch[col + 1u + 2u * j]));
}

/// Span `j`'s value (`way::Spans`).
fn span_val(col: u32, j: u32) -> u32 {
  return scratch[col + 2u + 2u * j];
}

/// The first cell from `start` toward `to`, a `step` at a time, that no slab or pier of
/// the column whose `n` spans begin at `col` fills (`way::bare`).
fn bare(col: u32, n: u32, cell: u32, start: i32, to: i32, step: i32) -> i32 {
  var z = start;
  var moved = true;
  while (moved && (z - to) * step <= 0i) {
    moved = false;
    for (var j = 0u; j < n; j += 1u) {
      let r = fill_range(cell, span_run(col, j));
      if ((span_val(col, j) & AIR_OF) == 0u && r.x <= z && z <= r.y) {
        z = select(r.x - 1i, r.y + 1i, step > 0i);
        moved = true;
      }
    }
  }
  return z;
}

/// The leaf cells between which column `c`'s cells at zoom `cell` hold what the ways
/// write there, its ground's top `top` leaf cells, off the spans in the scratch from `at`:
/// its own spans' cells, a bore's air as far as the top; and under its face cell, the
/// cells a bore's air beside it lines where no slab or pier of that column fills them
/// (`way::lined_hull`). Only a span of air lines a cell, so only its are walked.
fn lined_hull(at: u32, c: vec2<i32>, cell: u32, top: i32) -> vec2<i32> {
  let s = 1i << (LEAF_CELL - cell);
  let col = spans_at(at, c);
  let face = head_face(scratch[col]);
  let n = scratch[col] & SPANS_CNT;
  var lo = EMPTY_LO;
  var hi = EMPTY_HI;
  for (var j = 0u; j < n; j += 1u) {
    let r = fill_range(cell, span_run(col, j));
    var end = (r.y + 1i) * s;
    if ((span_val(col, j) & AIR_OF) != 0u) {
      end = min(end, top);
    }
    if (r.x <= r.y && r.x * s < end) {
      lo = min(lo, r.x * s);
      hi = max(hi, end);
    }
  }
  // A face of none lines nothing, and has no cell under it to name.
  for (var m = select(1u, 7u, face == FACE_NONE); m < 7u; m += 1u) {
    let d = near(m);
    let beside = spans_at(at, c + d.xy);
    let nb = scratch[beside] & SPANS_CNT;
    for (var j = 0u; j < nb; j += 1u) {
      if ((span_val(beside, j) & AIR_OF) == 0u) {
        continue;
      }
      // Air read at `z + d.z` lines `z`, under the face cell.
      let r = fill_range(cell, span_run(beside, j));
      let last = min(r.y, face - 1i + d.z);
      let first = bare(beside, nb, cell, r.x, last, 1i);
      if (first <= last) {
        let to = bare(beside, nb, cell, last, r.x, -1i);
        lo = min(lo, (first - d.z) * s);
        hi = max(hi, (to - d.z + 1i) * s);
      }
    }
  }
  return vec2<i32>(lo, hi);
}
