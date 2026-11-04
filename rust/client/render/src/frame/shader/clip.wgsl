// pxl-render/clip.wgsl

// Where the clip levels hold a cell (pxl-engine clipmap.rs).

fn spread(v: u32) -> u32 {
  var s = (v | (v << 8u)) & 0x00ff00ffu;
  s = (s | (s << 4u)) & 0x0f0f0f0fu;
  s = (s | (s << 2u)) & 0x33333333u;
  return (s | (s << 1u)) & 0x55555555u;
}

/// `clipmap::idx_of`.
fn cell_id(at: vec2<u32>) -> u32 {
  return spread(at.x) | (spread(at.y) << 1u);
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
    let m = vec2<i32>(i32(ACROSS) - 1i);
    let e = vec2<u32>(q & m);
    let row = clipmap[(z - FIRST_CELL) * CLIP_ENTRIES + e.y * ACROSS + e.x];
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
