// pxl-render/way.wgsl

// What the ways write in a cell, read from the spans a brick's fill kept on the CPU (pxl-engine
// clipmap.rs `way_at`, `way::way_cell`, `KeptSpans`). The GPU does no way geometry: a cell asks
// its column in the brick's run of kept spans, in constant time, and the rule over its spans.
// It keeps to scoped locals and plain loads; every loop runs over a column's spans, held to
// `WAY_SPANS`, or the six cells about one.

@group(0) @binding(7) var<storage, read> pool: array<u32>;

/// Where column `c`'s spans begin in the run from `run`, or 0 where it keeps none
/// (`clipmap::KeptSpans::column`): its bit, the columns before its pair of bitmap words, and
/// the bits before it.
fn run_column(run: u32, c: vec2<i32>) -> u32 {
  if (any(c < vec2<i32>(-1i)) || any(c > vec2<i32>(i32(BRICK)))) {
    return 0u;
  }
  let i = u32(c.y + 1i) * SPANS_ACROSS + u32(c.x + 1i);
  let w = i / 32u;
  let bit = i % 32u;
  let word = pool[run + RUN_BITS_AT + w];
  if (((word >> bit) & 1u) == 0u) {
    return 0u;
  }
  let rank = (pool[run + RUN_RANK_AT + w / 4u] >> (16u * ((w / 2u) & 1u))) & 0xffffu;
  let before = countOneBits(select(0u, pool[run + RUN_BITS_AT + w - 1u], (w & 1u) == 1u));
  return pool[run + RUN_OFFS_AT + rank + before + countOneBits(word & ((1u << bit) - 1u))];
}

/// A head word's spans' count, held to `WAY_SPANS` so that a loop over them is bounded whatever
/// it reads (`clipmap::head_of`).
fn head_cnt(w: u32) -> u32 {
  return min(w & SPANS_CNT, WAY_SPANS);
}

fn head_face(w: u32) -> i32 {
  return i32(w) >> 16u;
}

/// Whether the leaf cells `run`, `[lo, hi)`, fill cell `z` of zoom `cell` (pxl-spec `way::fills`).
fn way_fills(cell: u32, run: vec2<i32>, z: i32) -> bool {
  let s = 1i << (LEAF_CELL_ZOOM - cell);
  return 2i * (min(run.y, (z + 1i) * s) - max(run.x, z * s)) > s;
}

/// What cell `z` of the column whose spans begin at `at` holds (`way::at`): a deck's slab or its
/// pier, the higher keeping a shared cell, or else the first air over the cell (`WAY_AIR_OF`
/// over its way's palette index), or `WAY_NOTHING`.
fn column_at(run: u32, at: u32, cell: u32, z: i32) -> u32 {
  let n = head_cnt(pool[run + at]);
  var has = false;
  var best = vec2<i32>(0i);
  var palette = WAY_NOTHING;
  var air = WAY_NOTHING;
  for (var j = 0u; j < n; j += 1u) {
    let w = run + at + 1u + 2u * j;
    let span = bound_of(i32(pool[w]));
    if (way_fills(cell, span, z)) {
      let v = pool[w + 1u];
      if ((v & WAY_AIR_OF) != 0u) {
        if (air == WAY_NOTHING) {
          air = v;
        }
      } else if (!has || span.x > best.x || (span.x == best.x && span.y >= best.y)) {
        has = true;
        best = span;
        palette = v;
      }
    }
  }
  return select(air, palette, palette != WAY_NOTHING);
}

/// The `m`-th of the six cells about a cell, from the row under it (`way::near`).
fn way_near(m: u32) -> vec3<i32> {
  var i = 6u - m;
  var s = 1i;
  if (m < 4u) {
    i = m - 1u;
    s = -1i;
  }
  return select(vec3<i32>(0i), vec3<i32>(s), vec3<u32>(1u, 0u, 2u) == vec3<u32>(i));
}

/// What the ways write in cell `z` of column `c` of the run (`way::way_cell`): a deck's slab or
/// its pier first, or else a bore's air (`WAY_AIR`); under its face cell, otherwise, the palette
/// index of a bore's air beside it, the first of `way_near`; so a cave never opens into a way.
fn way_value(run: u32, cell: u32, c: vec2<i32>, z: i32) -> u32 {
  let own = run_column(run, c);
  if (own == 0u) {
    return WAY_NOTHING;
  }
  let v = column_at(run, own, cell, z);
  if (v != WAY_NOTHING) {
    return select(v, WAY_AIR, (v & WAY_AIR_OF) != 0u);
  }
  let face = head_face(pool[run + own]);
  if (face == FACE_NONE) {
    return WAY_NOTHING;
  }
  for (var m = 1u; z < face && m < 7u; m += 1u) {
    let d = way_near(m);
    let b = run_column(run, c + d.xy);
    if (b != 0u) {
      let beside = column_at(run, b, cell, z + d.z);
      if (beside != WAY_NOTHING && (beside & WAY_AIR_OF) != 0u) {
        return beside & ~WAY_AIR_OF;
      }
    }
  }
  return WAY_NOTHING;
}

/// What the ways write in the cell `c` at `zoom` asked of the brick its column was read from
/// (pxl-engine `clipmap::way_at`); `WAY_NOTHING` where nothing writes it. The column's bound,
/// `ways` (`way::lined_hull`), turns down the cells outside it.
fn way_at_cell(place: u32, ways: vec2<i32>, zoom: u32, c: vec3<i32>) -> u32 {
  let cell = lookup_from(place);
  if (zoom < cell) {
    return WAY_NOTHING;
  }
  let shift = zoom - cell;
  let z = c.z >> shift;
  let s = 1i << (LEAF_CELL_ZOOM - cell);
  if (z * s >= ways.y || (z + 1i) * s <= ways.x) {
    return WAY_NOTHING;
  }
  let run = ground[brick_at(lookup_slot(place)) + ROOT_AT];
  if (run == 0u) {
    return WAY_NOTHING;
  }
  let g = cell_at(zoom, cell, c.xy);
  return way_value(run, cell, vec2<i32>(vec2<u32>(g) & vec2<u32>(BRICK - 1u)), z);
}
