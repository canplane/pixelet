// @xpute/spec/lib/kit/word.rs
//
// NOTE: TS type aliases (u8, i32, u64 etc.), size constants (U8_SZ etc.),
// limit constants (U8_MAX, I32_MIN etc.), cast functions (U8(), U32() etc.),
// BOOL(), is_int, and Vec<N,T>/numeric/primitive/bits are intentionally omitted
// — Rust provides these natively or they are TS-specific type utilities.
// Async<T> is also omitted (lib/kit/type.ts) — use `impl Future` or `LocalBoxFuture` directly.

// ============ Fit Checks ============

pub fn fits_u8(x: i64) -> bool { x >= 0 && x <= u8::MAX as i64 }
pub fn fits_i8(x: i64) -> bool { x >= i8::MIN as i64 && x <= i8::MAX as i64 }
pub fn fits_u16(x: i64) -> bool { x >= 0 && x <= u16::MAX as i64 }
pub fn fits_i16(x: i64) -> bool { x >= i16::MIN as i64 && x <= i16::MAX as i64 }
pub fn fits_u32(x: i64) -> bool { x >= 0 && x <= u32::MAX as i64 }
pub fn fits_i32(x: i64) -> bool { x >= i32::MIN as i64 && x <= i32::MAX as i64 }

pub fn fits_u64(x: i128) -> bool { x >= 0 && x <= u64::MAX as i128 }
pub fn fits_i64(x: i128) -> bool { x >= i64::MIN as i128 && x <= i64::MAX as i128 }

pub fn fits_f32(x: f64) -> bool { x.is_finite() && (x as f32) as f64 == x }

// JS number is already IEEE-754 binary64.
// This check answers only whether the value may be lowered through the JS f64 path.
// It is NOT an integer-precision guarantee: large integral magnitudes may already
// have lost precision before reaching this predicate.
pub fn fits_f64(_x: f64) -> bool { true }

// ============ Bit Ops: 32-bit ============

pub const fn bit(nbit: u32) -> u32 { 1u32 << nbit } // k: 0..31
pub fn has_bit(u: u32, nbit: u32) -> bool { has_bits(u, bit(nbit)) }
pub fn has_bits(u: u32, bits: u32) -> bool { (u & bits) != 0 }
pub fn set_bits(u: u32, bits: u32) -> u32 { u | bits }
pub fn clr_bits(u: u32, bits: u32) -> u32 { u & !bits }

/// Returns a contiguous bit-field mask.
///
/// Semantics:
/// - nbit : field width in bits
/// - shamt: starting bit position
///
/// Examples:
/// - `field_mask(3, 0)` => `0b00000111`
/// - `field_mask(3, 5)` => `0b11100000`
///
/// Kernel rule:
/// - this helper is intentionally thin
/// - nbit/shamt validity is caller-owned
/// - 32-bit boundary / overshift cases are not guarded
pub const fn field_mask(nbit: u32, shamt: u32) -> u32 { ((1u32 << nbit) - 1) << shamt }
pub fn field_get(u: u32, shamt: u32, mask: u32) -> u32 { (u >> shamt) & mask }
pub fn field_set(u: u32, shamt: u32, mask: u32, v: u32) -> u32 {
    (u & !(mask << shamt)) | ((v & mask) << shamt)
}

// ============ Bit Ops: 64-bit ============

pub const fn bit64(shamt: u32) -> u64 { 1u64 << shamt } // k: 0..63
pub fn has_bit64(x: u64, shamt: u32) -> bool { has_bits64(x, bit64(shamt)) }
pub fn has_bits64(x: u64, bits: u64) -> bool { (x & bits) != 0 }
pub fn set_bits64(x: u64, bits: u64) -> u64 { x | bits }
pub fn clr_bits64(x: u64, bits: u64) -> u64 { x & !bits }

/// Returns a contiguous 64-bit field mask.
///
/// Semantics:
/// - nbit : field width in bits
/// - shamt: starting bit position
///
/// Examples:
/// - `field_mask64(3, 0)` => `0b111`
/// - `field_mask64(3, 5)` => `0b11100000`
///
/// Kernel rule:
/// - this helper is intentionally thin
/// - nbit/shamt validity is caller-owned
/// - 64-bit boundary / overshift cases are not guarded
pub const fn field_mask64(nbit: u32, shamt: u32) -> u64 { ((1u64 << nbit) - 1) << shamt }
pub fn field_get64(x: u64, shamt: u32, mask: u64) -> u64 { (x >> shamt) & mask }
pub fn field_set64(x: u64, shamt: u32, mask: u64, v: u64) -> u64 {
    (x & !(mask << shamt)) | ((v & mask) << shamt)
}
