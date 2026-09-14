// @pxl/shared/spec/space/pvf3/ivec.ts

//! PVF3 (Packed 3-Vector w/ Fixed-Point Arithmetic)
//! Integer Component Precision System
//!
//! This module provides atomic, stateless operations for the high-capacity
//! packed integer grid indexing. It handles the primary spatial partitioning
//! and discrete addressing logic for multi-layered coordinate systems.
//!
//! Memory Layout (N-bit Packed Index):
//! [ MSB ........................................................... LSB ]
//! [ Lane X (X_W bits) | Lane Y (Y_W bits) | Lane Z (Z_W bits) ]
//!
//! Example Configuration (64-bit Word):
//! [ 63 ................. 40 | 39 ................. 16 | 15 ................. 0 ]
//! [      Lane X (24b)       |      Lane Y (24b)       |      Lane Z (16b)       ]
//!
//! Mathematical Domain & Topology:
//! - X/Y/Z lanes are interpreted as signed (two's complement).
//! - X is cyclic (wrap-around) in the chosen world model: the signed boundary represents a continuous seam
//!   (i.e., MIN and MAX meet at the seam, e.g., antimeridian behavior) rather than a clamp.
//!
//! Operational Integrity & Overflow Policy:
//! - Deterministic Arithmetic: Uses BigInt for bit-exact operations; per-lane modular wrapping
//!   is enforced by masking in gather().
//! - Lane Isolation: Cross-lane carry is structurally impossible, as all arithmetic is performed
//!   on deconstructed lane values and re-packed via gather().
//! - Overflow Policy: Each lane wraps/truncates modulo 2^W; the caller must ensure semantic validity
//!   (e.g., displacements stay within lane-specific bit-depth when required by higher-level policies).
//! - Semantic Validity: 'Absolute + Absolute' addition is semantically invalid at the coordinate level
//!   and should be handled by the application.
//!
//! Format Scalability:
//! - This module is width-parametric: lane widths (X_W/Y_W/Z_W) and word width (WORD_W)
//!   define the packing. Increasing these bit-field sizes does not require changes to
//!   the core operations (gather/scatter/arithmetic); only the specification constants change.
//! - Any semantic interpretation (projection, cyclic seams, clamping, carry policy) is
//!   intentionally out of scope and must be implemented by the caller.
//!
//! The BigInt lane arithmetic is i64 here, wrapping: a lane is 24 bits at
//! most and a gather keeps only a lane's low bits, so the low 64 bits of the
//! unbounded result are what the word gets either way.

#![allow(non_snake_case)]

use super::core::{ABS, EQ, GT, LT, MAX, MIN};
use super::core::{IVEC_WORD_MASK as WORD_MASK, IVEC_X_W as X_W, IVEC_Y_W as Y_W, IVEC_Z_W as Z_W};

/// IVec: PVF3-Packed Integer Offset (Nominal Type)
/// Represents a discrete spatial unit within a multi-component grid.
pub type IVec = u64;

// ---- Bit Layout Configuration (Asymmetric Lane-based) ----

/// SHAMT: Bit-shift alignment for X-Y-Z sequential packing.
/// Calculated internally based on the core specification to isolate lanes.
const Z_SHAMT: u32 = 0;
const Y_SHAMT: u32 = Z_W;
const X_SHAMT: u32 = Z_W + Y_W;

const fn field_mask64(nbit: u32, shamt: u32) -> u64 {
    let field = if nbit >= 64 { u64::MAX } else { (1u64 << nbit) - 1 };
    if shamt >= 64 {
        0
    } else {
        field << shamt
    }
}

// ---- Component Axis Isolation Masks ----

pub const X_MASK: u64 = field_mask64(X_W, X_SHAMT);
pub const Y_MASK: u64 = field_mask64(Y_W, Y_SHAMT);
pub const Z_MASK: u64 = field_mask64(Z_W, Z_SHAMT);

/// `BigInt.asIntN(w, v)`: the low `w` bits of `v`, sign-extended.
const fn as_int_n(w: u32, v: u64) -> i64 {
    ((v << (64 - w)) as i64) >> (64 - w)
}

/// Axis Getters: Extract signed integer lanes.
/// Semantic domain follows the lane's signed two's-complement interpretation.
pub fn X(v: IVec) -> i64 {
    as_int_n(X_W, v >> X_SHAMT)
}
pub fn Y(v: IVec) -> i64 {
    as_int_n(Y_W, v >> Y_SHAMT)
}
pub fn Z(v: IVec) -> i64 {
    as_int_n(Z_W, v >> Z_SHAMT)
}

/// Integer scalar <-> real.
pub fn to_real(comp: i64) -> f64 {
    comp as f64
}
pub fn from_real(real: f64) -> i64 {
    real.floor() as i64
}

// ---- Logging ----

/// Human-readable representation of a packed IVec index/address.
/// Purely for logging and debugging; not for runtime logic.
pub fn repr(v: IVec) -> String {
    format!("{}:{}:{}", X(v), Y(v), Z(v))
}

// ---- Construction ----

/// Creates a new vector with all components initialized to a uniform value.
pub fn fill(e: i64) -> IVec {
    gather(e, e, e)
}

/// Assembles discrete components into a packed bit-field word.
/// Strict masking (& MASK) ensures no bit leakage between component fields.
pub fn gather(x: i64, y: i64, z: i64) -> IVec {
    let xn = ((x as u64).wrapping_shl(X_SHAMT)) & X_MASK;
    let yn = ((y as u64).wrapping_shl(Y_SHAMT)) & Y_MASK;
    let zn = ((z as u64).wrapping_shl(Z_SHAMT)) & Z_MASK;
    xn | yn | zn
}

/// Deconstructs a packed vector into its constituent components.
pub fn scatter(out: &mut [i64], v: IVec) {
    out[0] = X(v);
    out[1] = Y(v);
    out[2] = Z(v);
}

// ---- Bitwise ----

pub fn not(v: IVec) -> IVec {
    (!v) & WORD_MASK
}
pub fn and(v: IVec, w: IVec) -> IVec {
    v & w
}
pub fn or(v: IVec, w: IVec) -> IVec {
    v | w
}
pub fn xor(v: IVec, w: IVec) -> IVec {
    v ^ w
}

/// Signed shift: +k => shl, -k => shr (arith)
/// - ivec lanes are SIGNED (two's complement semantics via X/Y/Z sign-extension)
/// - right shift is arithmetic (sign-preserving) by construction of bigint >> on signed values
fn shl(v: IVec, shamt: i32) -> IVec {
    gather(X(v).wrapping_shl(shamt as u32), Y(v).wrapping_shl(shamt as u32), Z(v).wrapping_shl(shamt as u32))
}
fn shr(v: IVec, shamt: i32) -> IVec {
    gather(X(v) >> shamt, Y(v) >> shamt, Z(v) >> shamt)
}
pub fn sh(v: IVec, shamt: i32) -> IVec {
    if shamt >= 0 {
        shl(v, shamt)
    } else {
        shr(v, -shamt)
    }
}

// ---- Arithmetic ----

pub fn neg(v: IVec) -> IVec {
    gather(X(v).wrapping_neg(), Y(v).wrapping_neg(), Z(v).wrapping_neg())
}
pub fn abs(v: IVec) -> IVec {
    gather(ABS(X(v)), ABS(Y(v)), ABS(Z(v)))
}

pub fn add(v: IVec, w: IVec) -> IVec {
    gather(X(v).wrapping_add(X(w)), Y(v).wrapping_add(Y(w)), Z(v).wrapping_add(Z(w)))
}
pub fn sub(v: IVec, w: IVec) -> IVec {
    gather(X(v).wrapping_sub(X(w)), Y(v).wrapping_sub(Y(w)), Z(v).wrapping_sub(Z(w)))
}
pub fn mul(v: IVec, w: IVec) -> IVec {
    gather(X(v).wrapping_mul(X(w)), Y(v).wrapping_mul(Y(w)), Z(v).wrapping_mul(Z(w)))
}
pub fn div(v: IVec, w: IVec) -> IVec {
    gather(X(v) / X(w), Y(v) / Y(w), Z(v) / Z(w))
}
pub fn r#mod(v: IVec, w: IVec) -> IVec {
    gather(X(v) % X(w), Y(v) % Y(w), Z(v) % Z(w))
}

// ---- Comparison ----

pub fn eq(v: IVec, w: IVec) -> IVec {
    gather(EQ(X(v), X(w)), EQ(Y(v), Y(w)), EQ(Z(v), Z(w)))
}
pub fn equals(v: IVec, w: IVec) -> bool {
    v == w
}
pub fn lt(v: IVec, w: IVec) -> IVec {
    gather(LT(X(v), X(w)), LT(Y(v), Y(w)), LT(Z(v), Z(w)))
}
pub fn gt(v: IVec, w: IVec) -> IVec {
    gather(GT(X(v), X(w)), GT(Y(v), Y(w)), GT(Z(v), Z(w)))
}
pub fn max_comp(v: IVec) -> i64 {
    MAX(MAX(X(v), Y(v)), Z(v))
}
pub fn min_comp(v: IVec) -> i64 {
    MIN(MIN(X(v), Y(v)), Z(v))
}
pub fn max(v: IVec, w: IVec) -> IVec {
    gather(MAX(X(v), X(w)), MAX(Y(v), Y(w)), MAX(Z(v), Z(w)))
}
pub fn min(v: IVec, w: IVec) -> IVec {
    gather(MIN(X(v), X(w)), MIN(Y(v), Y(w)), MIN(Z(v), Z(w)))
}
pub fn clamp(v: IVec, min: i64, max: i64) -> IVec {
    gather(MIN(MAX(X(v), min), max), MIN(MAX(Y(v), min), max), MIN(MAX(Z(v), min), max))
}

// ---- Reduction ----

pub fn sum(v: IVec) -> i64 {
    X(v).wrapping_add(Y(v)).wrapping_add(Z(v))
}
pub fn dot(v: IVec, w: IVec) -> i64 {
    sum(mul(v, w))
}
pub fn norm1(v: IVec) -> i64 {
    sum(abs(v))
}
pub fn norm_sq(v: IVec) -> i64 {
    dot(v, v)
}
pub fn dist1(v: IVec, w: IVec) -> i64 {
    norm1(sub(v, w))
}
pub fn dist_sq(v: IVec, w: IVec) -> i64 {
    norm_sq(sub(v, w))
}
