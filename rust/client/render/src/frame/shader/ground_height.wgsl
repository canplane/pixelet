// pxl-render/ground_height.wgsl

// A column's height off its brick's tile's samples, as handed in its record (pxl-engine
// ground.rs `height`; clipmap.rs `ELE_AT`): what the tops' pass (height.wgsl) and the
// spans' (way.wgsl), which reads a ring column's, both read. Off `fresh`, the handed
// records, which each pass that takes it binds. Exact, in integers, the Rust's bits
// (i64.wgsl): every height is the Rust's.

/// Sample `i` of the brick's tile's padded samples, two a word, low first.
fn sample(rec: u32, i: u32) -> i32 {
  return (i32(fresh[rec + ELE_AT + i / 2u] >> (16u * (i % 2u))) << 16u) >> 16u;
}

/// A sample index on the padded grid, its edge's past it (`clamp_idx`).
fn clamp_idx(i: i32) -> u32 {
  return u32(clamp(i, 0i, i32(ELE_PADDED_SPAN) - 1i));
}

/// The first of the four samples along an axis about cell `g`'s center, and how far past
/// the second the center falls in `CELL_STEPS`ths of a sample (`window`).
fn window(g: i32) -> vec2<i32> {
  let s = 2i * g + 1i + i32(CELL_STEPS * ELE_PAD) - i32(CELL_STEPS / 2u);
  return vec2<i32>((s >> countTrailingZeros(CELL_STEPS)) - 1i, s & i32(CELL_STEPS - 1u));
}

/// Catmull-Rom's four weights at `m / CELL_STEPS` past the second sample, each times
/// `1 << WEIGHT_BITS` (`catmull_rom`).
fn catmull_rom(m: i32) -> vec4<i32> {
  let n = i32(CELL_STEPS);
  return vec4<i32>(
    -m * (n - m) * (n - m),
    3i * m * m * m - 5i * n * m * m + 2i * n * n * n,
    -3i * m * m * m + 4i * n * m * m + n * n * m,
    m * m * (m - n),
  );
}

/// The height the surface draws at the center of the tile's cell `g`, in leaf cells
/// (`ground::height`).
fn ground_height(rec: u32, g: vec2<i32>) -> i32 {
  let wx_at = window(g.x);
  let wy_at = window(g.y);
  let wx = catmull_rom(wx_at.y);
  let wy = catmull_rom(wy_at.y);
  var sum = vec2<u32>(0u);
  var lo = 2147483647i;
  var hi = -2147483647i - 1i;
  for (var j = 0u; j < 4u; j++) {
    let row_at = clamp_idx(wy_at.x + i32(j)) * ELE_PADDED_SPAN;
    var row = vec2<u32>(0u);
    for (var i = 0u; i < 4u; i++) {
      let v = sample(rec, row_at + clamp_idx(wx_at.x + i32(i)));
      lo = min(lo, v);
      hi = max(hi, v);
      row = i64_add(row, i64_of(v * wx[i]));
    }
    sum = i64_add(sum, i64_mul(row, i64_of(wy[j])));
  }
  // The high word is the floor, the low word the fraction: half to even.
  let under = i32(sum.y);
  let up = sum.x > 0x80000000u || (sum.x == 0x80000000u && (under & 1i) == 1i);
  return clamp(under + select(0i, 1i, up), lo, hi);
}
