// pxl-spec/space/pvf3/core.rs

//! The bit layout the three modules share — how wide a lane is, where the
//! fraction's one is — and the scalar comparisons a lane's arithmetic is
//! built from. Only widths are fixed here; each module derives its own shift
//! amounts and masks from them. A lane value is an `i64` that wraps: a lane
//! is 24 bits at most and a gather keeps only a lane's low bits, so the low
//! bits of a wrapped result are the lane's either way.

// Uppercase throughout, as the notation these operations are written in: one
// that holds no state and closes over nothing is named the way a C header
// names its macros. Rust's own answer to a macro like that is a `const fn`,
// which it cases snake, so the notation and the language disagree here and
// the file keeps the notation — suspended once for the file, which is the
// level the choice is made at rather than item by item.
#![allow(non_snake_case)]

use xpute_core::abi::word::{field_mask32, field_mask64};

// ---- IVec: Integer Part Specification ----

/// Total nominal bit-width for the packed integer word.
pub const IVEC_WORD_W: u32 = 64;
/// Mask for the entire packed word (e.g., 64-bit full mask)
pub const IVEC_WORD_MASK: u64 = field_mask64(IVEC_WORD_W, 0);
/// Bit-depth resolution assigned to Lane X.
pub const IVEC_X_W: u32 = 24;
/// Bit-depth resolution assigned to Lane Y.
pub const IVEC_Y_W: u32 = 24;
/// Bit-depth resolution assigned to Lane Z.
pub const IVEC_Z_W: u32 = 16;

// ---- FVec: Fractional Part Specification ----

/// Total nominal bit-width for the packed fractional word.
pub const FVEC_WORD_W: u32 = 32;
/// Symmetric bit-depth resolution assigned per fractional lane.
pub const FRAC_W: u32 = 8;
/// Mask for the 3 packed lanes (24 bits used of 32).
pub const FVEC_WORD_MASK: u32 = field_mask32(FRAC_W * 3, 0);
/// Cardinality of one fractional lane (2^FRAC_W).
pub const ONE: u64 = 1 << FRAC_W;
/// Low fractional-lane bit mask.
pub const ONE_MASK: u64 = ONE - 1;

// ---- Atomic Bitwise Utilities ----

const FALSE: i64 = 0;
const TRUE: i64 = !FALSE;
pub fn ABS(e: i64) -> i64 {
    if e < 0 {
        e.wrapping_neg()
    } else {
        e
    }
}
pub fn EQ(a: i64, b: i64) -> i64 {
    if a == b {
        TRUE
    } else {
        FALSE
    }
}
pub fn LT(a: i64, b: i64) -> i64 {
    if a < b {
        TRUE
    } else {
        FALSE
    }
}
pub fn GT(a: i64, b: i64) -> i64 {
    if a > b {
        TRUE
    } else {
        FALSE
    }
}
pub fn MAX(a: i64, b: i64) -> i64 {
    if a > b {
        a
    } else {
        b
    }
}
pub fn MIN(a: i64, b: i64) -> i64 {
    if a < b {
        a
    } else {
        b
    }
}
