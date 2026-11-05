// pxl-render/ground_height.wgsl

// A column's height off its brick's tile's samples, as handed in its record (pxl-engine
// ground.rs `height`, `bicubic`; clipmap.rs `ELE_AT`): what the tops' pass (height.wgsl) and
// the spans' (way.wgsl), which reads a ring column's, both read. Off `fresh`, the handed
// records, which each pass that takes it binds.
//
// The surface is Catmull-Rom over sixteen samples in f32, each product and sum in the
// Rust's order. A GPU that fuses a product into its sum rounds once where the Rust rounds
// twice, so a height whose sum falls within a few ulps of a half may land on the other
// whole cell (tests/fill.rs `on_edge`); every other height is the Rust's.

/// Sample `i` of the brick's tile's padded samples, two a word, low first.
fn sample(rec: u32, i: u32) -> i32 {
  return (i32(fresh[rec + ELE_AT + i / 2u] >> (16u * (i % 2u))) << 16u) >> 16u;
}

/// A sample index on the padded grid, its edge's past it (`clamp_idx`).
fn clamp_idx(i: i32) -> u32 {
  return u32(clamp(i, 0i, i32(ELE_PADDED_SPAN) - 1i));
}

/// Catmull-Rom's four weights at `t` past the second sample.
fn catmull_rom(t: f32) -> vec4<f32> {
  let t2 = t * t;
  let t3 = t * t * t;
  return vec4<f32>(
    -0.5 * t3 + t2 - 0.5 * t,
    1.5 * t3 - 2.5 * t2 + 1.0,
    -1.5 * t3 + 2.0 * t2 + 0.5 * t,
    0.5 * t3 - 0.5 * t2,
  );
}

/// The height the surface draws at the center of the tile's cell `g`, in leaf cells
/// (`ground::height`, `bicubic`).
fn ground_height(rec: u32, g: vec2<i32>) -> i32 {
  let s = (vec2<f32>(g) + CELL_CENTER) * (1.0 / CELL_STEP) + f32(ELE_PAD) - 0.5;
  let f = floor(s);
  let first = vec2<i32>(f) - 1i;
  let wx = catmull_rom(s.x - f.x);
  let wy = catmull_rom(s.y - f.y);
  var out = 0.0;
  var lo = 2147483647i;
  var hi = -2147483647i - 1i;
  for (var j = 0u; j < 4u; j++) {
    let row_at = clamp_idx(first.y + i32(j)) * ELE_PADDED_SPAN;
    var row = 0.0;
    for (var i = 0u; i < 4u; i++) {
      let v = sample(rec, row_at + clamp_idx(first.x + i32(i)));
      lo = min(lo, v);
      hi = max(hi, v);
      row += f32(v) * wx[i];
    }
    out += row * wy[j];
  }
  return i32(round(clamp(out, f32(lo), f32(hi))));
}
