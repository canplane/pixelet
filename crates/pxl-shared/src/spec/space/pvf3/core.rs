// @pxl/shared/spec/space/pvf3/core.ts

//! PVF3 (Packed 3-Vector w/ Fixed-Point Arithmetic)
//! Central Metadata Configuration
//!
//! This module defines the raw bit-width specifications and fundamental bitwise
//! utilities for the PVF3 system. It serves as the single source of truth for
//! canonical bit-layout specifications across all generalized vector kernels.
//!
//! Design Philosophy:
//! - Domain Agnostic: This system is a pure numerical framework. No assumptions
//!   are made about the coordinate domain (e.g., Geographic or Local).
//! - Width-Parametric Specification: All width constants are adjustable to fit
//!   specific spatial requirements and word-size constraints.
//! - Functional Encapsulation: Only raw widths are defined here. Shift amounts
//!   (SHAMT) and masks are derived within individual kernels to maintain
//!   internal operational autonomy and configuration flexibility.
//! - Deterministic Arithmetic: All utilities operate in the BigInt domain to
//!   ensure bit-exact behavior and avoid floating-point side effects — an
//!   i64 here, wrapping where the BigInt is unbounded, since a lane's low
//!   bits are what a gather keeps.

#![allow(non_snake_case)]

/// `FIELD_MASK64(nbit, shamt)`: a contiguous 64-bit field mask, a width of
/// 64 being the whole word.
const fn field_mask64(nbit: u32, shamt: u32) -> u64 {
    let field = if nbit >= 64 { u64::MAX } else { (1u64 << nbit) - 1 };
    if shamt >= 64 {
        0
    } else {
        field << shamt
    }
}

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
pub const FVEC_WORD_MASK: u64 = field_mask64(FRAC_W * 3, 0);
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

#[cfg(test)]
#[path = "core.test.rs"]
mod test;
