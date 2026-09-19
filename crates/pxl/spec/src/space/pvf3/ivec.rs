// pxl-spec/space/pvf3/ivec.rs

//! The integer lanes: one `u64` holding a signed 24-bit X, a signed 24-bit Y
//! and a signed 16-bit Z, the cell index a position has.
//!
//! [ 63 ................. 40 | 39 ................. 16 | 15 ................. 0 ]
//! [      Lane X (24b)       |      Lane Y (24b)       |      Lane Z (16b)       ]
//!
//! - Lanes are two's complement. X is cyclic: its signed boundary is a seam
//!   where MIN meets MAX, the antimeridian, rather than a clamp.
//! - Every operation unpacks the lanes, computes on each as an `i64`, and
//!   repacks through `gather`, which masks each result to its lane. So a
//!   lane wraps modulo 2^W, and no carry crosses from one lane to the next.
//!   Whether a wrapped result means anything — a displacement that fits, an
//!   absolute plus an absolute — is the caller's to know.
//! - The widths are `core`'s constants; nothing here depends on their values
//!   beyond fitting a `u64`. What a lane means — a projection, the seam, a
//!   clamp — is not decided here.

// Uppercase throughout, as the notation these operations are written in: one
// that holds no state and closes over nothing is named the way a C header
// names its macros. Rust's own answer to a macro like that is a `const fn`,
// which it cases snake, so the notation and the language disagree here and
// the file keeps the notation — suspended once for the file, which is the
// level the choice is made at rather than item by item.
#![allow(non_snake_case)]

use xpute_core::abi::word::{as_int_n64, field_mask64};

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

// ---- Component Axis Isolation Masks ----

pub const X_MASK: u64 = field_mask64(X_W, X_SHAMT);
pub const Y_MASK: u64 = field_mask64(Y_W, Y_SHAMT);
pub const Z_MASK: u64 = field_mask64(Z_W, Z_SHAMT);

/// Axis Getters: Extract signed integer lanes.
/// Semantic domain follows the lane's signed two's-complement interpretation.
pub fn X(v: IVec) -> i64 {
    as_int_n64(X_W, v >> X_SHAMT)
}
pub fn Y(v: IVec) -> i64 {
    as_int_n64(Y_W, v >> Y_SHAMT)
}
pub fn Z(v: IVec) -> i64 {
    as_int_n64(Z_W, v >> Z_SHAMT)
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

/// Signed shift: +k => shl, -k => shr. A lane is sign-extended by X/Y/Z,
/// so the right shift on its `i64` is arithmetic.
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
