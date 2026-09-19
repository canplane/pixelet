// pxl-spec/space/pvf3/fvec.rs

//! The fraction lanes: one word holding an unsigned 8-bit X, Y and Z, the
//! position within a cell in 256ths.
//!
//! [ 31 .. 24 | 23 ....... 16 | 15 ....... 8 | 7 ........ 0 ]
//! [ Reserved |  Lane X (8b) |  Lane Y (8b) |  Lane Z (8b) ]
//!
//! - A lane is a residue in [0, ONE): it carries no sign, and the direction
//!   belongs to `ivec`. Arithmetic is in the residue domain, wrapping modulo
//!   2^FRAC_W through `gather`; a carry into the integer lane is not made
//!   here, and a caller that needs one handles the boundary before packing.
//! - A residue maps to a real in [0, 1): a position within the cell at no
//!   physical scale.
//! - `FRAC_W` is `core`'s; the arithmetic does not depend on its value.

// Uppercase throughout, as the notation these operations are written in: one
// that holds no state and closes over nothing is named the way a C header
// names its macros. Rust's own answer to a macro like that is a `const fn`,
// which it cases snake, so the notation and the language disagree here and
// the file keeps the notation — suspended once for the file, which is the
// level the choice is made at rather than item by item.
#![allow(non_snake_case)]

use xpute_core::abi::word::field_mask32;

use super::core::{EQ, GT, LT, MAX, MIN};
use super::core::{FRAC_W, FVEC_WORD_MASK as WORD_MASK, ONE, ONE_MASK};

/// FVec: PVF3-Packed Fractional Residue Word (Nominal Type)
/// Represents packed unsigned fractional residues within a single lattice unit.
pub type FVec = u32;

// ---- Bit Layout Configuration (Symmetric Lane-based) ----

/// SHAMT: Bit-shift alignment for X-Y-Z sequential packing.
/// Calculated internally based on the core specification.
const Z_SHAMT: u32 = 0;
const Y_SHAMT: u32 = Z_SHAMT + FRAC_W;
const X_SHAMT: u32 = Y_SHAMT + FRAC_W;

// ---- Component Axis Isolation Masks ----

const X_MASK: u32 = field_mask32(FRAC_W, X_SHAMT);
const Y_MASK: u32 = field_mask32(FRAC_W, Y_SHAMT);
const Z_MASK: u32 = field_mask32(FRAC_W, Z_SHAMT);

/// Axis Getters: Extract residue lanes.
/// Returned as i64 for scalar-surface uniformity;
/// semantic domain remains [0, ONE).
pub fn X(v: FVec) -> i64 {
    ((v & X_MASK) >> X_SHAMT) as i64
}
pub fn Y(v: FVec) -> i64 {
    ((v & Y_MASK) >> Y_SHAMT) as i64
}
pub fn Z(v: FVec) -> i64 {
    ((v & Z_MASK) >> Z_SHAMT) as i64
}

/// Fractional residue scalar <-> normalized real.
/// - to_real:   residue scalar -> [0, 1)
/// - from_real: real -> [0, 1) modulo 1 -> residue scalar
pub fn to_real(comp: i64) -> f64 {
    (comp as u64 & ONE_MASK) as f64 / ONE as f64
}
pub fn from_real(real: f64) -> i64 {
    let alpha = ((real % 1.0) + 1.0) % 1.0;
    (ONE as f64 * alpha).floor() as i64
}

// ---- Logging ----

/// Human-readable representation of a packed FVec.
/// Purely for logging and debugging; not for runtime logic.
pub fn repr(v: FVec) -> String {
    format!("{}:{}:{}", X(v), Y(v), Z(v))
}

// ---- Construction ----

/// Creates a new vector with all components initialized to a uniform value.
pub fn fill(e: i64) -> FVec {
    gather(e, e, e)
}

/// Assembles discrete components into a packed bit-field word.
/// Strict masking (& MASK) ensures no bit leakage between component fields.
pub fn gather(x: i64, y: i64, z: i64) -> FVec {
    let xn = ((x as u32).wrapping_shl(X_SHAMT)) & X_MASK;
    let yn = ((y as u32).wrapping_shl(Y_SHAMT)) & Y_MASK;
    let zn = ((z as u32).wrapping_shl(Z_SHAMT)) & Z_MASK;
    xn | yn | zn
}

/// Deconstructs a packed vector into its constituent components.
pub fn scatter(out: &mut [i64], v: FVec) {
    out[0] = X(v);
    out[1] = Y(v);
    out[2] = Z(v);
}

// ---- Bitwise ----

pub fn not(v: FVec) -> FVec {
    (!v) & WORD_MASK
}
pub fn and(v: FVec, w: FVec) -> FVec {
    v & w
}
pub fn or(v: FVec, w: FVec) -> FVec {
    v | w
}
pub fn xor(v: FVec, w: FVec) -> FVec {
    v ^ w
}

/// Signed shift: +k => shl, -k => shr. A lane is a non-negative residue,
/// so the arithmetic right shift on its `i64` is a logical one.
fn shl(v: FVec, shamt: i32) -> FVec {
    gather(X(v).wrapping_shl(shamt as u32), Y(v).wrapping_shl(shamt as u32), Z(v).wrapping_shl(shamt as u32))
}
fn shr(v: FVec, shamt: i32) -> FVec {
    gather(X(v) >> shamt, Y(v) >> shamt, Z(v) >> shamt)
}
pub fn sh(v: FVec, shamt: i32) -> FVec {
    if shamt >= 0 {
        shl(v, shamt)
    } else {
        shr(v, -shamt)
    }
}

// ---- Arithmetic ----
// NOTE(fvec):
// - lanes are UNSIGNED residues in [0, 2^FRAC_W), i.e. [0, ONE).
// - signed-like neg/abs are intentionally omitted from this surface.
// - if signed semantics are needed, use ivec or reconstruct a higher-level scalar first.

pub fn add(v: FVec, w: FVec) -> FVec {
    gather(X(v).wrapping_add(X(w)), Y(v).wrapping_add(Y(w)), Z(v).wrapping_add(Z(w)))
}
pub fn sub(v: FVec, w: FVec) -> FVec {
    gather(X(v).wrapping_sub(X(w)), Y(v).wrapping_sub(Y(w)), Z(v).wrapping_sub(Z(w)))
}
pub fn mul(v: FVec, w: FVec) -> FVec {
    gather(X(v).wrapping_mul(X(w)), Y(v).wrapping_mul(Y(w)), Z(v).wrapping_mul(Z(w)))
}
pub fn div(v: FVec, w: FVec) -> FVec {
    gather(X(v) / X(w), Y(v) / Y(w), Z(v) / Z(w))
}
pub fn r#mod(v: FVec, w: FVec) -> FVec {
    gather(X(v) % X(w), Y(v) % Y(w), Z(v) % Z(w))
}

// ---- Comparison ----

pub fn eq(v: FVec, w: FVec) -> FVec {
    gather(EQ(X(v), X(w)), EQ(Y(v), Y(w)), EQ(Z(v), Z(w)))
}
pub fn equals(v: FVec, w: FVec) -> bool {
    v == w
}
pub fn lt(v: FVec, w: FVec) -> FVec {
    gather(LT(X(v), X(w)), LT(Y(v), Y(w)), LT(Z(v), Z(w)))
}
pub fn gt(v: FVec, w: FVec) -> FVec {
    gather(GT(X(v), X(w)), GT(Y(v), Y(w)), GT(Z(v), Z(w)))
}
pub fn max_comp(v: FVec) -> i64 {
    MAX(MAX(X(v), Y(v)), Z(v))
}
pub fn min_comp(v: FVec) -> i64 {
    MIN(MIN(X(v), Y(v)), Z(v))
}
pub fn max(v: FVec, w: FVec) -> FVec {
    gather(MAX(X(v), X(w)), MAX(Y(v), Y(w)), MAX(Z(v), Z(w)))
}
pub fn min(v: FVec, w: FVec) -> FVec {
    gather(MIN(X(v), X(w)), MIN(Y(v), Y(w)), MIN(Z(v), Z(w)))
}
pub fn clamp(v: FVec, min: i64, max: i64) -> FVec {
    gather(MIN(MAX(X(v), min), max), MIN(MAX(Y(v), min), max), MIN(MAX(Z(v), min), max))
}

// ---- Reduction ----
// NOTE(fvec):
// - These reduction helpers are residue-domain proxies, not geometric metrics.
// - norm1 is identical to the component sum in the unsigned residue domain.
// - dist1/dist_sq operate over wrapped subtraction, so they should not be
//   interpreted as canonical spatial distances.
// - these helpers may also be used as local L0-in-L1 slot metrics/orderings
//   in subcell traversal or SVO-style local indexing, not only as numeric proxies.

pub fn sum(v: FVec) -> i64 {
    X(v) + Y(v) + Z(v)
}
pub fn dot(v: FVec, w: FVec) -> i64 {
    sum(mul(v, w))
}
pub fn norm1(v: FVec) -> i64 {
    X(v) + Y(v) + Z(v) // residue-domain proxy: identical to component sum
}
pub fn norm_sq(v: FVec) -> i64 {
    dot(v, v)
}
pub fn dist1(v: FVec, w: FVec) -> i64 {
    norm1(sub(v, w)) // residue-domain proxy over wrapped subtraction
}
pub fn dist_sq(v: FVec, w: FVec) -> i64 {
    norm_sq(sub(v, w))
}
