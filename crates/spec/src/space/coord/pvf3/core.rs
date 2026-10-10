// pxl-spec/space/coord/pvf3/core.rs

//! The lane widths the PVF3 modules share and the scalar comparisons their
//! arithmetic uses. A lane value is an `i64` that wraps; `gather` keeps only
//! a lane's low bits, so the result is the lane's either way.

// Uppercase as the notation these macro-like helpers are written in.
#![allow(non_snake_case)]

use xpute_core::abi::word::{field_mask32, field_mask64};

pub const IVEC_WORD_W: u32 = 64;
pub const IVEC_WORD_MASK: u64 = field_mask64(IVEC_WORD_W, 0);
pub const IVEC_X_W: u32 = 24;
pub const IVEC_Y_W: u32 = 24;
pub const IVEC_Z_W: u32 = 16;

pub const FVEC_WORD_W: u32 = 32;
pub const FRAC_W: u32 = 8;
/// The three packed lanes (24 bits used of 32).
pub const FVEC_WORD_MASK: u32 = field_mask32(FRAC_W * 3, 0);
pub const ONE: u64 = 1 << FRAC_W;
pub const ONE_MASK: u64 = ONE - 1;

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
