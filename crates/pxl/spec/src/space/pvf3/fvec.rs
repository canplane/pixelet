// pxl-spec/space/pvf3/fvec.rs

//! The fraction lanes: one word holding unsigned 8-bit X, Y and Z, the
//! position within a cell in 256ths.
//!
//! [ 31 .. 24 | 23 ....... 16 | 15 ....... 8 | 7 ........ 0 ]
//! [ Reserved |  Lane X (8b) |  Lane Y (8b) |  Lane Z (8b) ]
//!
//! A lane is an unsigned residue wrapping modulo 2^FRAC_W; no carry into the
//! integer lane is made here.

// Uppercase as the notation these macro-like helpers are written in.
#![allow(non_snake_case)]

use xpute_core::abi::word::field_mask32;

use super::core::{EQ, GT, LT, MAX, MIN};
use super::core::{FRAC_W, FVEC_WORD_MASK as WORD_MASK, ONE, ONE_MASK};

pub type FVec = u32;

const Z_SHAMT: u32 = 0;
const Y_SHAMT: u32 = Z_SHAMT + FRAC_W;
const X_SHAMT: u32 = Y_SHAMT + FRAC_W;

const X_MASK: u32 = field_mask32(FRAC_W, X_SHAMT);
const Y_MASK: u32 = field_mask32(FRAC_W, Y_SHAMT);
const Z_MASK: u32 = field_mask32(FRAC_W, Z_SHAMT);

/// Residues in [0, ONE), as i64.
pub fn X(v: FVec) -> i64 {
    ((v & X_MASK) >> X_SHAMT) as i64
}
pub fn Y(v: FVec) -> i64 {
    ((v & Y_MASK) >> Y_SHAMT) as i64
}
pub fn Z(v: FVec) -> i64 {
    ((v & Z_MASK) >> Z_SHAMT) as i64
}

pub fn to_real(comp: i64) -> f64 {
    (comp as u64 & ONE_MASK) as f64 / ONE as f64
}
pub fn from_real(real: f64) -> i64 {
    let alpha = ((real % 1.0) + 1.0) % 1.0;
    (ONE as f64 * alpha).floor() as i64
}

/// For logging only.
pub fn repr(v: FVec) -> String {
    format!("{}:{}:{}", X(v), Y(v), Z(v))
}

pub fn fill(e: i64) -> FVec {
    gather(e, e, e)
}

pub fn gather(x: i64, y: i64, z: i64) -> FVec {
    let xn = ((x as u32).wrapping_shl(X_SHAMT)) & X_MASK;
    let yn = ((y as u32).wrapping_shl(Y_SHAMT)) & Y_MASK;
    let zn = ((z as u32).wrapping_shl(Z_SHAMT)) & Z_MASK;
    xn | yn | zn
}

pub fn scatter(out: &mut [i64], v: FVec) {
    out[0] = X(v);
    out[1] = Y(v);
    out[2] = Z(v);
}

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

/// +k shifts left, -k right; a lane is non-negative, so the right shift is logical.
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

// Lanes are unsigned, so there is no neg or abs.

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

// Residue-domain proxies over wrapped subtraction, not spatial distances.

pub fn sum(v: FVec) -> i64 {
    X(v) + Y(v) + Z(v)
}
pub fn dot(v: FVec, w: FVec) -> i64 {
    sum(mul(v, w))
}
pub fn norm1(v: FVec) -> i64 {
    X(v) + Y(v) + Z(v)
}
pub fn norm_sq(v: FVec) -> i64 {
    dot(v, v)
}
pub fn dist1(v: FVec, w: FVec) -> i64 {
    norm1(sub(v, w))
}
pub fn dist_sq(v: FVec, w: FVec) -> i64 {
    norm_sq(sub(v, w))
}
