// @pxl/spec/space/pvf3/core.rs
//
// PVF3 Central Metadata Configuration.
// Single source of truth for bit-layout specifications across all pvf3 kernels.

// ---- IVec: Integer Part Specification ----

pub const IVEC_WORD_W: u32 = 64;
pub const IVEC_WORD_MASK: u64 = u64::MAX; // FIELD_MASK64(64)
pub const IVEC_X_W: u32 = 24;
pub const IVEC_Y_W: u32 = 24;
pub const IVEC_Z_W: u32 = 16;

// TODO(guard-bits): The current 24-24-16 packing fills all 64 bits with no room for
// guard bits. This forces every lane operation to unpack → operate → repack, which is
// 3× the instruction count of direct packed arithmetic.
//
// A guard-bit layout would reserve one extra bit per lane so that addition carry stays
// inside each lane and never bleeds across boundaries. Two candidates:
//
//   Option A — trim each lane by 1 bit:
//     [ G | X (23b) | G | Y (23b) | G | Z (15b) | pad (1b) ] = 64 bits total
//     (X range halves: ±4M → ±8M but usable range already limited by the 3857 adapter)
//
//   Option B — use a 128-bit word (two u64) as the packed form:
//     IVec = u128 with generous guard bits per lane; costs more memory but avoids
//     any lane-width reduction.
//
// With guard bits, add/sub/sh can operate directly on the packed word without unpacking,
// and the guard bit doubles as a carry/overflow flag — critical for detecting X-lane
// wrap-around at the antimeridian seam (cyclic boundary) when computing deltas.

// ---- FVec: Fractional Part Specification ----

pub const FVEC_WORD_W: u32 = 32;
pub const FVEC_WORD_MASK: u64 = 0xFFFF_FFFF; // FIELD_MASK64(32)
pub const FRAC_W: u32 = 10;
pub const ONE: u64 = 1 << FRAC_W; // 1024
pub const ONE_MASK: u64 = ONE - 1; // 0x3FF

// ---- Bit Utilities ----

/// Packed field mask: ((1 << w) - 1) << shamt.
pub const fn field_mask64(w: u32, shamt: u32) -> u64 {
    if w == 64 {
        u64::MAX.wrapping_shl(shamt)
    } else {
        ((1u64 << w) - 1) << shamt
    }
}

/// Single bit: 1 << w.
pub const fn bit64(w: u32) -> u64 {
    1u64 << w
}

// ---- Atomic Utilities ----
// These map directly to TS: ABS, EQ, LT, GT, MAX, MIN.
// EQ/LT/GT return i64 boolean masks: -1 (all bits set) for true, 0 for false.

#[inline] pub fn abs_i64(e: i64) -> i64 { e.wrapping_abs() }
#[inline] pub fn eq_mask(a: i64, b: i64) -> i64 { if a == b { -1 } else { 0 } }
#[inline] pub fn lt_mask(a: i64, b: i64) -> i64 { if a < b { -1 } else { 0 } }
#[inline] pub fn gt_mask(a: i64, b: i64) -> i64 { if a > b { -1 } else { 0 } }
#[inline] pub fn max_i64(a: i64, b: i64) -> i64 { a.max(b) }
#[inline] pub fn min_i64(a: i64, b: i64) -> i64 { a.min(b) }
