// pxl-render/bound.wgsl

// Level `level.x` of the bricks `fresh` names, one a `z`, a thread an entry (pxl-engine
// fill.rs `level`): level 0 each column's own, its top and the leaf cells its ways write
// off the spans its fill kept (spans.wgsl); a higher one the join of the four under it,
// written by the dispatch before (`clipmap::join`). A dispatch a level, from 0 up.

@group(0) @binding(0) var<uniform> level: vec4<u32>;
@group(0) @binding(1) var<storage, read> fresh: array<u32>;
@group(0) @binding(2) var<storage, read_write> ground: array<u32>;
@group(0) @binding(3) var<storage, read> scratch: array<u32>;

/// Entry `at` of level `j` of the brick at `base`, handed in the record at `rec` as the
/// `k`-th, as its two bound words.
fn entry_bounds(base: u32, j: u32, at: vec2<u32>, rec: u32, k: u32) -> vec2<i32> {
  if (j == 0u) {
    let top = column_top(ground[base + COLUMNS_AT + cell_id(at)]);
    let spans = scratch_at(arrayLength(&scratch), k);
    let lines = lined_hull(spans, vec2<i32>(at), fresh[rec + FILL_CELL], top);
    return bounds_word(vec4<i32>(top, top, lines));
  }
  var b = BOUNDS_EMPTY;
  for (var q = 0u; q < 4u; q += 1u) {
    let e = level_entry(base, j - 1u, 2u * at + vec2<u32>(q & 1u, q >> 1u));
    b = joined(b, bounds_of(i32(ground[e]), i32(ground[e + 1u])));
  }
  return bounds_word(b);
}

@compute @workgroup_size(LEVEL_SIDE, LEVEL_SIDE, 1)
fn bound_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let j = level.x;
  if (id.x >= BRICK >> j || id.y >= BRICK >> j) {
    return;
  }
  let rec = handed_at(id.z);
  let base = brick_at(fresh[rec + FILL_BRICK]);
  let w = entry_bounds(base, j, id.xy, rec, id.z);
  let e = level_entry(base, j, id.xy);
  ground[e] = u32(w.x);
  ground[e + 1u] = u32(w.y);
}
