// pxl-render/decor_scatter.wgsl

// The decor the ground holds, off the hashes alone (pxl-content scatter.rs): how many a column
// holds, a leaf cell's candidate, and whether it stands clear of the decor's others. No world
// and no state: a thread asks a leaf cell by its name.

/// `share::quarter`.
fn share_quarter(n: u32, room: u32, across: u32, along: u32, q: u32) -> u32 {
  let a = halve(n, 2u * room, 2u * room, across);
  let half = select(n - a, a, (q & 1u) == 0u);
  let b = halve(half, room, room, along);
  return select(half - b, b, (q & 2u) == 0u);
}

/// `scatter::count`: how many of decor `k` the column `b` at `zoom` holds.
fn scatter_count(k: u32, zoom: u32, b: vec2<i32>) -> u32 {
  return scatter_count_of(row_word(k, ROW_SEED), row_word(k, ROW_CHANCE), decor_root(k), zoom, b);
}

/// `scatter::count_of`: `scatter_count` of a decor of `seed`, `chance` and `root`.
fn scatter_count_of(seed: u32, chance: u32, root: u32, zoom: u32, b: vec2<i32>) -> u32 {
  if (zoom < root) {
    return 0u;
  }
  let mean = (chance >> 16u) << (2u * (LEAF_CELL - root));
  let first = quad_seed(seed ^ COUNT_TAG, root, b >> vec2<u32>(zoom - root));
  var n = (mean + (first & 0xffffu)) >> 16u;
  var z = root;
  while (z < zoom && n > 0u) {
    let h = quad_seed(seed ^ SPLIT_TAG, z, b >> vec2<u32>(zoom - z));
    let c = vec2<u32>(b >> vec2<u32>(zoom - z - 1u)) & vec2<u32>(1u);
    let room = min(1u << (2u * (LEAF_CELL - z - 1u)), 0xffffu);
    n = share_quarter(n, room, draw(h, 0u), draw(h, 1u), c.x | (c.y << 1u));
    z += 1u;
  }
  return n;
}

/// The first draw of leaf cell `leaf`'s candidate of decor `k`, and the seed of its own draws
/// after it, as `scatter::candidate_at` draws them (`Candidate`).
fn candidate_draws(k: u32, leaf: vec2<i32>) -> vec2<u32> {
  let h = quad_seed(row_word(k, ROW_SEED), LEAF_CELL, leaf);
  return vec2<u32>(draw(h, 0u), draw(h, 1u));
}

/// `scatter::variant`.
fn variant_of(k: u32, seed: u32) -> u32 {
  return mul_hi(draw(seed, 0u), row_word(k, ROW_VARIANTS));
}

/// `scatter::side_of`.
fn side_of(k: u32, seed: u32) -> i32 {
  let size = cells(k, variant_of(k, seed), 0u);
  return max(size.x, size.y);
}

/// `scatter::stands_at`, a unit's decor: its box keeps an eighth of a cell at `cell`.
fn stands_at(k: u32, variant: u32, cell: u32) -> bool {
  let size = cells(k, variant, 0u);
  let steps = 3u * (UNIT_ZOOM - min(UNIT_ZOOM, cell));
  return cell >= LEAF_CELL || (steps < 32u && ((EIGHTHS * u32(size.x * size.y * size.z)) >> steps) >= 1u);
}

/// `scatter::retained`: no candidate before it (`before`) claims a brick meeting its own
/// (`meet`), of the cells a brick meeting its may start at.
fn retained(k: u32, leaf: vec2<i32>, first: u32, seed: u32) -> bool {
  let side = side_of(k, seed);
  let extent = decor_widest(k) - 1i;
  for (var y = leaf.y - extent; y < leaf.y + side; y += 1i) {
    for (var x = leaf.x - extent; x < leaf.x + side; x += 1i) {
      if ((x != leaf.x || y != leaf.y) && scatter_count(k, LEAF_CELL, vec2<i32>(x, y)) > 0u) {
        let d = candidate_draws(k, vec2<i32>(x, y));
        let before = d.x < first || (d.x == first && (y < leaf.y || (y == leaf.y && x < leaf.x)));
        if (before) {
          let other = side_of(k, d.y);
          if (x < leaf.x + side && leaf.x < x + other && y < leaf.y + side && leaf.y < y + other) {
            return false;
          }
        }
      }
    }
  }
  return true;
}
