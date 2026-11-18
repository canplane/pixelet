// pxl-render/frame/shader/world_rule.wgsl

// The world's rules that read no buffer, written by hand as twins of their Rust
// functions: a fill's eighths, the service area's wall, a bound word, the bounds a clip level's
// entry holds, and an elevation in cells. The world's query (world.wgsl) reads them, and a
// value's kind, which build.rs writes into world_word (`is_ground`, `voxel`).

// A fill's eighths, as pxl-spec share.rs and pxl-engine answer.rs state them.

fn eighths(fill: u32) -> u32 {
  return min(fill / (FULL / EIGHTH_CNT), EIGHTH_CNT);
}

// The service area's wall, as pxl-spec area.rs states it.

/// The port of pxl-spec `area::is_wall`, read from `shard`, the shard of the node
/// `(cell_zoom, c)`.
fn is_wall(shard: u32, cell_zoom: u32, c: vec2<i32>) -> bool {
  let served = shard >> SHARD_SERVED_SHIFT;
  if (cell_zoom < LEAF_CELL_ZOOM) {
    return false;
  }
  let leaf = c >> vec2<u32>(cell_zoom - LEAF_CELL_ZOOM);
  let own = c >> vec2<u32>(cell_zoom - SHARD_ZOOM);
  let shift = LEAF_CELL_ZOOM - SHARD_ZOOM;
  // The shards of the leaf cells around it, at most two on each side.
  let lo = ((leaf - 1i) >> vec2<u32>(shift)) - own;
  let hi = ((leaf + 1i) >> vec2<u32>(shift)) - own;
  let dx = vec2<i32>(lo.x, hi.x) + 1i;
  let dy = vec2<i32>(lo.y, hi.y) + 1i;
  let bits = vec4<u32>(vec4<i32>(dy.x * 3i + dx.x, dy.x * 3i + dx.y, dy.y * 3i + dx.x, dy.y * 3i + dx.y));
  return any(((vec4<u32>(served) >> bits) & vec4<u32>(1u)) == vec4<u32>(0u));
}

// The bounds a clip level's entry holds (pxl-engine clipmap.rs), packed as pxl-engine answer.rs
// and ground.rs pack them.

/// The lowest and the highest a bound word holds (pxl-engine `answer::bound_of`).
fn bound_of(w: i32) -> vec2<i32> {
  return vec2<i32>((w << 16u) >> 16u, w >> 16u);
}

/// A bound packed into a word, its lowest in the low half and its highest in the high, each as
/// an i16 (pxl-engine `answer::bound_word`).
fn bound_word(run: vec2<i32>) -> u32 {
  return u32((run.x & 0xffffi) | (run.y << 16u));
}

/// The bounds two bound words hold (pxl-engine `ground::extent_of`, `ground::Extent`): the
/// ground's elevations, lowest and highest, then the leaf cells the ways fill, lowest and past the
/// highest.
fn extent_of(a: i32, b: i32) -> vec4<i32> {
  return vec4<i32>(bound_of(a), bound_of(b));
}

/// The bounds `b` as their two bound words (pxl-engine `ground::extent_word`).
fn extent_word(b: vec4<i32>) -> vec2<i32> {
  return vec2<i32>(i32(bound_word(b.xy)), i32(bound_word(b.zw)));
}

/// Two bounds joined, each source on its own: the lower of the two lowests and the higher of
/// the two highests (pxl-engine `ground::joined`).
fn joined(a: vec4<i32>, b: vec4<i32>) -> vec4<i32> {
  return vec4<i32>(min(a.x, b.x), max(a.y, b.y), min(a.z, b.z), max(a.w, b.w));
}

const BOUNDS_EMPTY = vec4<i32>(EMPTY_LO, EMPTY_HI, EMPTY_LO, EMPTY_HI);
const BOUNDS_OPEN = vec4<i32>(UNBOUNDED_LO, UNBOUNDED_HI, UNBOUNDED_LO, UNBOUNDED_HI);

// An elevation in cells, as pxl-spec ground.rs states it.

struct EleCells {
  cells: i32,
  part: u32,
}

/// The port of pxl-spec `ground::ele_cells` for an elevation of `ele` leaf cells: the top cell, and the
/// share of it filled in `FULL`'s units. Coarser than the leaves, the top is the cell the
/// elevation ends in where it fills an eighth of that cell or more, and otherwise the whole cell
/// under it.
fn ele_cells(cell_zoom: u32, ele: i32) -> EleCells {
  if (cell_zoom >= LEAF_CELL_ZOOM) {
    return EleCells(ele << (cell_zoom - LEAF_CELL_ZOOM), FULL);
  }
  let n = LEAF_CELL_ZOOM - cell_zoom;
  let cells = ele >> n;
  let rest = u32(ele) & ((1u << n) - 1u);
  let part = select(rest >> ((n - FULL_BITS) & 31u), rest << ((FULL_BITS - n) & 31u), n <= FULL_BITS);
  if (eighths(part) == 0u) {
    return EleCells(cells, FULL);
  }
  return EleCells(cells + 1i, part);
}

/// The port of pxl-spec `ground::ele_drawn`: the elevation drawn for `ele_cells` cells, in leaf
/// cells. Finer than the leaves, it is the leaf cell the top's cell lies in, rounded down under
/// sea level as over it. Coarser, it is the cells scaled.
fn ele_drawn(cell_zoom: u32, ele_cells: i32) -> i32 {
  if (cell_zoom >= LEAF_CELL_ZOOM) {
    return ele_cells >> (cell_zoom - LEAF_CELL_ZOOM);
  }
  return ele_cells << (LEAF_CELL_ZOOM - cell_zoom);
}

fn leaf_cells(cell_zoom: u32) -> i32 {
  return select(1i, 1i << ((cell_zoom - LEAF_CELL_ZOOM) & 31u), cell_zoom >= LEAF_CELL_ZOOM);
}
