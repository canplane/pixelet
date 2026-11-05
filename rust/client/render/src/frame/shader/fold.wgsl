// pxl-render/fold.wgsl

// The eye's clip levels' levels over their bricks (pxl-engine clipmap.rs `fold`), a thread
// an entry, each a row of the table past its entries (`clipmap::JOINED_AT`), which a node
// wider than a brick reads as one entry (clip.wgsl `held_of`). A level a dispatch, each
// over the level under it, which the dispatch before folded. Run where the clip levels'
// slots moved; a present brick's levels stand while it does.

@group(0) @binding(1) var<storage, read_write> clipmap: array<vec4<i32>>;
@group(0) @binding(2) var<storage, read> ground: array<u32>;

/// Quarter `k` of entry `b` of level `l` of the clip level at `cell`, whose first brick is
/// `first`: at level 1 a brick's own levels, off the finest clip level holding it present
/// (`held_brick`), none (`BOUNDS_OPEN`) where none does; over it, the entry of the level
/// under it, none where its key names another first brick (`unfold`).
fn quarter(cell: u32, l: u32, b: u32, k: u32, first: vec2<i32>) -> vec4<i32> {
  if (l == 1u) {
    let at = (id_cell(b) << vec2<u32>(l)) + vec2<u32>(k & 1u, k >> 1u);
    let q = clipmap[entry_row(cell, entry(vec2<i32>(at)))].xy;
    let n = bounded(LOOKUP_NONE, cell, cell - BRICK_LEVELS, q);
    return select(BOUNDS_OPEN, n.b, n.here);
  }
  return unfold(clipmap[joined_row(cell, joined_idx(l - 1u, vec2<u32>(0u)) + 4u * b + k)], first);
}

/// Entry `b` of level `l` of the eye's clip level at `cell` (`clipmap::fold`): keyed by
/// the clip level's first brick, its four quarters' bounds joined; the zero entry where
/// the clip level is not laid out or a quarter holds none.
fn fold(cell: u32, l: u32, b: u32) -> vec4<i32> {
  let origin = clipmap[ORIGIN_AT + cell - FIRST_CELL];
  if (origin.z == 0i) {
    return vec4<i32>(0i);
  }
  var joins = BOUNDS_EMPTY;
  for (var k = 0u; k < 4u; k += 1u) {
    let got = quarter(cell, l, b, k, origin.xy);
    if (got.x == UNBOUNDED_LO && got.y == UNBOUNDED_HI) {
      return vec4<i32>(0i);
    }
    joins = joined(joins, got);
  }
  let w = bounds_word(joins);
  return vec4<i32>(i32(key_word(origin.x)), i32(key_word(origin.y)), w.x, w.y);
}

// The level is the dispatch's depth in workgroups: only its last slice folds, the
// levels under it folded before.
@compute @workgroup_size(FOLD_LANES)
fn fold_cs(@builtin(global_invocation_id) id: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>) {
  let l = groups.z;
  let blocks = (ACROSS >> l) * (ACROSS >> l);
  if (id.z + 1u != l || id.x >= blocks * EYE_CLIP_LEVELS) {
    return;
  }
  let cell = FIRST_CELL + id.x / blocks;
  let b = id.x % blocks;
  clipmap[joined_row(cell, joined_idx(l, vec2<u32>(0u)) + b)] = fold(cell, l, b);
}
