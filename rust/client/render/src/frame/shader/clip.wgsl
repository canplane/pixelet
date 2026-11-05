// pxl-render/clip.wgsl

// Where the clip levels hold a node's cells, and what they bound under it at a zoom
// (pxl-engine clipmap.rs `step`, `bounded`): off their table, `clipmap`, which a pass that
// asks the world binds read only (clipmap.wgsl) and the fold binds to write (fold.wgsl),
// and their bricks' levels, `ground`.

fn spread(v: u32) -> u32 {
  var s = (v | (v << 8u)) & 0x00ff00ffu;
  s = (s | (s << 4u)) & 0x0f0f0f0fu;
  s = (s | (s << 2u)) & 0x33333333u;
  return (s | (s << 1u)) & 0x55555555u;
}

/// `spread`'s inverse (`clipmap::gather`).
fn gather(v: u32) -> u32 {
  var g = v & 0x55555555u;
  g = (g | (g >> 1u)) & 0x33333333u;
  g = (g | (g >> 2u)) & 0x0f0f0f0fu;
  g = (g | (g >> 4u)) & 0x00ff00ffu;
  return (g | (g >> 8u)) & 0x0000ffffu;
}

/// `clipmap::idx_of`.
fn cell_id(at: vec2<u32>) -> u32 {
  return spread(at.x) | (spread(at.y) << 1u);
}

/// `clipmap::of_idx`.
fn id_cell(i: u32) -> vec2<u32> {
  return vec2<u32>(gather(i), gather(i >> 1u));
}

/// The entry brick `q` is held in (`clipmap::entry`).
fn entry(q: vec2<i32>) -> u32 {
  let e = vec2<u32>(q & vec2<i32>(i32(ACROSS) - 1i));
  return e.y * ACROSS + e.x;
}

/// The row of entry `e` of the clip level at `cell` in the table.
fn entry_row(cell: u32, e: u32) -> u32 {
  return (cell - FIRST_CELL) * CLIP_ENTRIES + e;
}

/// `clipmap::key_word`.
fn key_word(v: i32) -> u32 {
  return u32(v) ^ 0x80000000u;
}

/// `clipmap::joined_idx`.
fn joined_idx(l: u32, at: vec2<u32>) -> u32 {
  let under = ACROSS >> (l - 1u);
  return (ACROSS * ACROSS - under * under) / 3u + cell_id(at);
}

/// The row of entry `e` of the eye's clip level at `cell` over its bricks
/// (`clipmap::JOINED_AT`).
fn joined_row(cell: u32, e: u32) -> u32 {
  return JOINED_AT + (cell - FIRST_CELL) * JOINED_ENTRIES + e;
}

/// `clipmap::unfold`: the bounds the entry `e` holds of a clip level whose first brick is
/// `first`, none where its key names another.
fn unfold(e: vec4<i32>, first: vec2<i32>) -> vec4<i32> {
  if (u32(e.x) == key_word(first.x) && u32(e.y) == key_word(first.y)) {
    return bounds_of(e.z, e.w);
  }
  return BOUNDS_OPEN;
}

fn lookup(cell: u32, clip: u32, slot: u32, here: bool) -> u32 {
  return (cell << LOOKUP_CELL_SHIFT) | (clip << LOOKUP_FROM_SHIFT) | (u32(here) << LOOKUP_SLOT_BITS) | slot;
}

fn lookup_cell(l: u32) -> u32 {
  return l >> LOOKUP_CELL_SHIFT;
}

fn lookup_from(l: u32) -> u32 {
  return (l >> LOOKUP_FROM_SHIFT) & ((1u << ZOOM_BITS) - 1u);
}

fn lookup_slot(l: u32) -> u32 {
  return l & ((1u << LOOKUP_SLOT_BITS) - 1u);
}

fn lookup_here(l: u32) -> bool {
  return (l & (1u << LOOKUP_SLOT_BITS)) != 0u;
}

/// `clipmap::cell_at`.
fn cell_at(zoom: u32, clip: u32, c: vec2<i32>) -> vec2<i32> {
  let s = zoom - clip;
  return vec2<i32>(wrap_idx(clip, c.x >> s), c.y >> s);
}

struct Step {
  place: u32,
  at: vec2<u32>,
}

/// `clipmap::step`: the zooms `above` leaves unknown, finest first.
fn step_to(above: u32, cell: u32, g: vec2<i32>) -> Step {
  let known = above != LOOKUP_NONE && cell >= lookup_from(above);
  var stop = FIRST_CELL - 1u;
  var f = lookup(cell, FIRST_CELL, 0u, false);
  if (known) {
    stop = lookup_cell(above);
    f = (above & LOOKUP_AT) | (cell << LOOKUP_CELL_SHIFT);
  }
  for (var z = cell; z > stop; z -= 1u) {
    let q = cell_at(cell, z, g) >> vec2<u32>(BRICK_SHIFT);
    let row = clipmap[entry_row(z, entry(q))];
    if (row.z != 0i && row.x == q.x && row.y == q.y) {
      f = lookup(cell, z, u32(row.z - 1i), true);
      break;
    }
  }
  return Step(f, vec2<u32>(cell_at(cell, lookup_from(f), g) & vec2<i32>(i32(BRICK) - 1i)));
}

fn held_to(cell: u32) -> u32 {
  return clamp(cell, FIRST_CELL, LEAF_CELL);
}

/// What the clip levels hold of a node (`clipmap::bounded`, `clipmap::bound_over`): its
/// bounds, the clip level read, whether one held it, whether that one holds all of it, and
/// where its cells were found.
struct NodeBound {
  b: vec4<i32>,
  clip: u32,
  here: bool,
  all: bool,
  place: u32,
}

/// `clipmap::held_of`: the eye's clip level at `cell` of a node `2^l` bricks a side.
fn held_of(place: u32, cell: u32, g: vec2<i32>, l: u32) -> NodeBound {
  let origin = clipmap[ORIGIN_AT + cell - FIRST_CELL];
  if (origin.z == 0i) {
    return NodeBound(BOUNDS_EMPTY, cell, true, false, place);
  }
  let q = vec2<i32>(wrap_idx(cell, g.x), g.y) >> vec2<u32>(BRICK_SHIFT);
  let d = vec2<i32>(wrap_idx(cell - BRICK_SHIFT, q.x - origin.x), q.y - origin.y);
  let side = 1i << l;
  let across = i32(ACROSS);
  if (d.x >= across || d.y >= across || d.x + side <= 0i || d.y + side <= 0i) {
    return NodeBound(BOUNDS_EMPTY, cell, true, false, place);
  }
  let level = min(l, JOINED_LEVELS);
  let at = vec2<u32>(q & vec2<i32>(across - 1i)) >> vec2<u32>(level);
  let b = unfold(clipmap[joined_row(cell, joined_idx(level, at))], origin.xy);
  // A brick no clip level holds: no coarser clip level bounds it either.
  if (b.x == UNBOUNDED_LO && b.y == UNBOUNDED_HI) {
    return NodeBound(b, cell, true, true, place);
  }
  return NodeBound(b, cell, true, all(d >= vec2<i32>(0i)) && all(d + side <= vec2<i32>(across)), place);
}

/// `clipmap::bounded`.
fn bounded(place: u32, cell: u32, zoom: u32, c: vec2<i32>) -> NodeBound {
  // The node's first cell at `cell`, or the cell holding it (`idx_at`).
  let g = select(c >> vec2<u32>((zoom - cell) & 31u), c << vec2<u32>((cell - zoom) & 31u), cell >= zoom);
  if (cell > zoom + BRICK_LEVELS) {
    return held_of(place, cell, g, cell - zoom - BRICK_LEVELS);
  }
  let s = step_to(place, cell, g);
  let clip = lookup_from(s.place);
  if (!lookup_here(s.place)) {
    return NodeBound(vec4<i32>(0i), clip, false, false, s.place);
  }
  let j = select(0u, clip - zoom, zoom < clip);
  let e = level_entry(brick_at(lookup_slot(s.place)), j, s.at >> vec2<u32>(j));
  return NodeBound(bounds_of(i32(ground[e]), i32(ground[e + 1u])), clip, true, true, s.place);
}
