// pxl-render/i64.wgsl

// Signed 64-bit integers, which WGSL has none of: a value a `vec2<u32>`, its low word then
// its high, in two's complement, for the rules the CPU and a GPU decide alike in integers
// whose products pass 32 bits (pxl-engine ground.rs `height`, way.rs `on_way`, `crossing`,
// `pier_covers`, where the Rust takes `i64`). Adding, subtracting and multiplying wrap as
// `i64`'s do, so a value is the Rust's while it stays within 2^63, as each rule's bits keep
// it.

fn i64_of(a: i32) -> vec2<u32> {
  return vec2<u32>(u32(a), u32(a >> 31u));
}

fn i64_add(a: vec2<u32>, b: vec2<u32>) -> vec2<u32> {
  let lo = a.x + b.x;
  return vec2<u32>(lo, a.y + b.y + select(0u, 1u, lo < a.x));
}

fn i64_sub(a: vec2<u32>, b: vec2<u32>) -> vec2<u32> {
  return vec2<u32>(a.x - b.x, a.y - b.y - select(0u, 1u, a.x < b.x));
}

/// `a · b`: the low words' whole product, its high word off their halves (decor_row.wgsl
/// `mul_hi`), and each high word times the other's low word over it.
fn i64_mul(a: vec2<u32>, b: vec2<u32>) -> vec2<u32> {
  let low = (a.x & 0xffffu) * (b.x & 0xffffu);
  let cross = vec2<u32>((a.x >> 16u) * (b.x & 0xffffu), (a.x & 0xffffu) * (b.x >> 16u));
  let carry = (low >> 16u) + (cross.x & 0xffffu) + (cross.y & 0xffffu);
  let hi = (a.x >> 16u) * (b.x >> 16u) + (cross.x >> 16u) + (cross.y >> 16u) + (carry >> 16u);
  return vec2<u32>(a.x * b.x, hi + a.x * b.y + a.y * b.x);
}

/// `a · b` of two `i32`.
fn i64_mul32(a: i32, b: i32) -> vec2<u32> {
  return i64_mul(i64_of(a), i64_of(b));
}

fn i64_lt(a: vec2<u32>, b: vec2<u32>) -> bool {
  return i32(a.y) < i32(b.y) || (a.y == b.y && a.x < b.x);
}

fn i64_neg(a: vec2<u32>) -> vec2<u32> {
  return i64_sub(vec2<u32>(0u), a);
}

fn i64_abs(a: vec2<u32>) -> vec2<u32> {
  return select(a, i64_neg(a), i32(a.y) < 0i);
}

/// Near `a`, within a float's rounding: what an estimate starts from, which an exact
/// product then corrects.
fn i64_f32(a: vec2<u32>) -> f32 {
  return f32(i32(a.y)) * 4294967296.0 + f32(a.x);
}
