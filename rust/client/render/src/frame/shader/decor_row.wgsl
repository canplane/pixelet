// pxl-render/decor_row.wgsl

// A decor as the catalog's row holds it, read a word at a time, and the draws its seeds start
// (pxl-spec asset/decor.spec.rs, scatter.rs; xpute-kit `SplitMix32`).

@group(0) @binding(10) var<storage, read> rows: array<u32>;

fn row_word(k: u32, at: u32) -> u32 {
  return rows[k * ROW_WORDS + at];
}

/// `SplitMix32`'s `n`-th draw off `seed`.
fn draw(seed: u32, n: u32) -> u32 {
  return seed_mix32(seed + n * SEED_SALT_32, 0u);
}

/// This is the port of pxl-content `scatter::mul_hi`.
fn mul_hi(a: u32, b: u32) -> u32 {
  let low = (a & 0xffffu) * (b & 0xffffu);
  let cross = vec2<u32>((a >> 16u) * (b & 0xffffu), (a & 0xffffu) * (b >> 16u));
  let carry = (low >> 16u) + (cross.x & 0xffffu) + (cross.y & 0xffffu);
  return (a >> 16u) * (b >> 16u) + (cross.x >> 16u) + (cross.y >> 16u) + (carry >> 16u);
}

/// `Decor::size`: `variant`'s box, turned `turn` quarter turns.
fn decor_size(k: u32, variant: u32, turn: u32) -> vec3<i32> {
  let at = ROW_SIZES + 3u * variant;
  let x = i32(row_word(k, at));
  let y = i32(row_word(k, at + 1u));
  let z = i32(row_word(k, at + 2u));
  return select(vec3<i32>(y, x, z), vec3<i32>(x, y, z), (turn & 1u) == 0u);
}

/// This is the port of pxl-content `scatter::variant_of`.
fn variant_of(k: u32, seed: u32) -> u32 {
  return mul_hi(draw(seed, 0u), row_word(k, ROW_VARIANT_CNT));
}

/// This is the port of pxl-content `Decor::allows`.
fn allows(k: u32, v: u32) -> bool {
  return v < 256u && ((row_word(k, ROW_ON + (v >> 5u)) >> (v & 31u)) & 1u) == 1u;
}
