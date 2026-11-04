// pxl-render/world_rule.wgsl

// The world's rules that read no buffer, by hand: a value's kind, the service area's wall,
// a brick's cell order, a bound word and a column's top in cells. What the world's query
// (world.wgsl) and the ways' fill (way.wgsl, way_tree.wgsl) both read.

// ---- value.rs, answer.rs ----

fn grounds(v: u32) -> bool {
  return v >= GROUNDS_FIRST && v <= GROUNDS_LAST;
}

fn voxel(v: u32) -> bool {
  return v == VALUE_AIR || grounds(v) || (v >= OBJECTS_FIRST && v <= OBJECTS_LAST);
}

fn eighths(fill: u32) -> u32 {
  return min(fill / (FULL / EIGHTHS), EIGHTHS);
}

// ---- area.rs ----

/// `area::is_wall`, off the shard the node `(cell, b)` knows.
fn is_wall(shard: u32, cell: u32, b: vec2<i32>) -> bool {
  let served = shard >> SHARD_SERVED_SHIFT;
  if (cell < LEAF_CELL) {
    return false;
  }
  let leaf = b >> vec2<u32>(cell - LEAF_CELL);
  let own = b >> vec2<u32>(cell - SHARD_ZOOM);
  let shift = LEAF_CELL - SHARD_ZOOM;
  // The shards of the leaf cells about it: two a side at most.
  let lo = ((leaf - 1i) >> vec2<u32>(shift)) - own;
  let hi = ((leaf + 1i) >> vec2<u32>(shift)) - own;
  let dx = vec2<i32>(lo.x, hi.x) + 1i;
  let dy = vec2<i32>(lo.y, hi.y) + 1i;
  let bits = vec4<u32>(vec4<i32>(dy.x * 3i + dx.x, dy.x * 3i + dx.y, dy.y * 3i + dx.x, dy.y * 3i + dx.y));
  return any(((vec4<u32>(served) >> bits) & vec4<u32>(1u)) == vec4<u32>(0u));
}

// ---- clipmap.rs ----

/// A bound word's lowest and highest (`answer::bound_of`).
fn bound_of(w: i32) -> vec2<i32> {
  return vec2<i32>((w << 16u) >> 16u, w >> 16u);
}

/// `ground::Bounds`: the ground's lowest and highest, then the ways'.
fn bounds_of(a: i32, b: i32) -> vec4<i32> {
  return vec4<i32>(bound_of(a), bound_of(b));
}

// ---- ground.rs ----

struct Top {
  cells: i32,
  part: u32,
}

/// `ground::top` of `h` leaf cells.
fn top_of(cell: u32, h: i32) -> Top {
  if (cell >= LEAF_CELL) {
    return Top(h << (cell - LEAF_CELL), FULL);
  }
  let n = LEAF_CELL - cell;
  let cells = h >> n;
  let rest = u32(h) & ((1u << n) - 1u);
  let part = select(rest >> ((n - FULL_BITS) & 31u), rest << ((FULL_BITS - n) & 31u), n <= FULL_BITS);
  if (eighths(part) == 0u) {
    return Top(cells, FULL);
  }
  return Top(cells + 1i, part);
}

fn face_height(cell: u32, t: i32) -> i32 {
  if (cell >= LEAF_CELL) {
    return t >> (cell - LEAF_CELL);
  }
  return t << (LEAF_CELL - cell);
}

fn leaf_cells(cell: u32) -> i32 {
  return select(1i, 1i << ((cell - LEAF_CELL) & 31u), cell >= LEAF_CELL);
}
