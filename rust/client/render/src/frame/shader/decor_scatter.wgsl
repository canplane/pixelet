// pxl-render/frame/shader/decor_scatter.wgsl

// The decor the ground holds, found from the hashes alone (pxl-content scatter.rs): how many of
// a decor a cell holds, a leaf cell's candidate, and whether it stands clear of the decor's
// others. It reads no world and keeps no state, so a thread asks of a leaf cell by its name
// alone.

/// This is the port of pxl-content `halve::quarter`.
fn quarter(n: u32, cap: u32, w: vec2<u32>, q: u32) -> u32 {
  let a = halve(n, vec2<u32>(2u * cap), w.x);
  let half = select(n - a, a, (q & 1u) == 0u);
  let b = halve(half, vec2<u32>(cap), w.y);
  return select(half - b, b, (q & 2u) == 0u);
}

/// This is the port of pxl-content `scatter::near_half`.
fn near_half(w: u32) -> u32 {
  return weight(0x7000u + ((w >> 16u) & 0x1fffu), w);
}

/// `scatter::cnt`: how many of decor `k` the cell `b` at `zoom` holds.
fn scatter_cnt(k: u32, zoom: u32, b: vec2<i32>) -> u32 {
  return scatter_cnt_of(row_word(k, ROW_SEED), row_word(k, ROW_CHANCE), decor_root(k), zoom, b);
}

/// `scatter::cnt_of`: `scatter_cnt` of a decor of `seed`, `chance` and `root`.
fn scatter_cnt_of(seed: u32, chance: u32, root: u32, zoom: u32, b: vec2<i32>) -> u32 {
  if (zoom < root) {
    return 0u;
  }
  let mean = (chance >> 16u) << (2u * (LEAF_CELL_ZOOM - root));
  let first = quad_seed(seed ^ SCATTER_CNT_TAG, root, b >> vec2<u32>(zoom - root));
  var n = (mean + (first & 0xffffu)) >> 16u;
  var z = root;
  while (z < zoom && n > 0u) {
    let h = quad_seed(seed ^ SCATTER_SPLIT_TAG, z, b >> vec2<u32>(zoom - z));
    let c = vec2<u32>(b >> vec2<u32>(zoom - z - 1u)) & vec2<u32>(1u);
    let cap = min(1u << (2u * (LEAF_CELL_ZOOM - z - 1u)), 0xffffu);
    let along = select(draw(h, 1u), draw(h, 2u), c.x == 1u);
    n = quarter(n, cap, vec2<u32>(near_half(draw(h, 0u)), near_half(along)), c.x | (c.y << 1u));
    z += 1u;
  }
  return n;
}

/// The port of pxl-content `scatter::Candidate`.
struct Candidate {
  cell: vec2<i32>,
  draw: u32,
  seed: u32,
}

/// Leaf cell `cell`'s candidate of decor `k`, as `scatter::candidate_at` draws it.
fn candidate_at(k: u32, cell: vec2<i32>) -> Candidate {
  let h = quad_seed(row_word(k, ROW_SEED), LEAF_CELL_ZOOM, cell);
  return Candidate(cell, draw(h, 0u), draw(h, 1u));
}

/// This is the port of pxl-content `scatter::side_of`.
fn side_of(k: u32, seed: u32) -> i32 {
  let size = decor_size(k, variant_of(k, seed), 0u);
  return max(size.x, size.y);
}

/// `scatter::shows_at` for a decor sized in unit cells: whether its box keeps an eighth of a
/// cell at `cell_zoom` or more. At the leaves' clip level it always shows.
fn shows_at(k: u32, variant: u32, cell_zoom: u32) -> bool {
  let size = decor_size(k, variant, 0u);
  let steps = 3u * (UNIT_ZOOM - min(UNIT_ZOOM, cell_zoom));
  return cell_zoom >= LEAF_CELL_ZOOM || (steps < 32u && ((EIGHTH_CNT * u32(size.x * size.y * size.z)) >> steps) >= 1u);
}

/// `scatter::retained`: whether no candidate before it (`before`) claims a brick that meets its
/// own (`meet`). It asks every cell at which a brick meeting its own may start.
fn retained(k: u32, c: Candidate) -> bool {
  let side = side_of(k, c.seed);
  let extent = decor_widest(k) - 1i;
  for (var y = c.cell.y - extent; y < c.cell.y + side; y += 1i) {
    for (var x = c.cell.x - extent; x < c.cell.x + side; x += 1i) {
      if ((x != c.cell.x || y != c.cell.y) && scatter_cnt(k, LEAF_CELL_ZOOM, vec2<i32>(x, y)) > 0u) {
        let d = candidate_at(k, vec2<i32>(x, y));
        let before = d.draw < c.draw || (d.draw == c.draw && (y < c.cell.y || (y == c.cell.y && x < c.cell.x)));
        if (before) {
          let other = side_of(k, d.seed);
          if (x < c.cell.x + side && c.cell.x < x + other && y < c.cell.y + side && c.cell.y < y + other) {
            return false;
          }
        }
      }
    }
  }
  return true;
}
