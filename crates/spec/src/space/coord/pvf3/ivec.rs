// pxl-spec/space/coord/pvf3/ivec.rs

//! The integer lanes: one `u64` holding a signed 24-bit X, a signed 24-bit Y
//! and a signed 16-bit Z, the cell index a position has.
//!
//! [ 63 ................. 40 | 39 ................. 16 | 15 ................. 0 ]
//! [      Lane X (24b)       |      Lane Y (24b)       |      Lane Z (16b)       ]
//!
//! Lanes are two's complement, and X's boundary is the antimeridian seam.
//! Every operation repacks through `gather`, so a lane wraps modulo 2^W and
//! no carry crosses lanes.

// Uppercase as the notation these macro-like helpers are written in.
#![allow(non_snake_case)]

use xpute_kit::abi::word::{as_int_n64, field_mask64};

use crate::space::coord::v3::V3;

use super::core::{ABS, EQ, GT, LT, MAX, MIN};
use super::core::{IVEC_WORD_MASK as WORD_MASK, IVEC_X_W as X_W, IVEC_Y_W as Y_W, IVEC_Z_W as Z_W};

pub type IVec = u64;

const Z_SHAMT: u32 = 0;
const Y_SHAMT: u32 = Z_W;
const X_SHAMT: u32 = Z_W + Y_W;

pub const X_MASK: u64 = field_mask64(X_W, X_SHAMT);
pub const Y_MASK: u64 = field_mask64(Y_W, Y_SHAMT);
pub const Z_MASK: u64 = field_mask64(Z_W, Z_SHAMT);

pub fn X(v: IVec) -> i64 {
    as_int_n64(X_W, v >> X_SHAMT)
}
pub fn Y(v: IVec) -> i64 {
    as_int_n64(Y_W, v >> Y_SHAMT)
}
pub fn Z(v: IVec) -> i64 {
    as_int_n64(Z_W, v >> Z_SHAMT)
}

pub fn to_real(comp: i64) -> f64 {
    comp as f64
}
pub fn to_v3(v: IVec) -> V3 {
    [to_real(X(v)), to_real(Y(v)), to_real(Z(v))]
}
pub fn from_real(real: f64) -> i64 {
    real.floor() as i64
}

/// For logging only.
pub fn repr(v: IVec) -> String {
    format!("{}:{}:{}", X(v), Y(v), Z(v))
}

pub fn fill(e: i64) -> IVec {
    gather(e, e, e)
}

pub fn gather(x: i64, y: i64, z: i64) -> IVec {
    let xn = ((x as u64).wrapping_shl(X_SHAMT)) & X_MASK;
    let yn = ((y as u64).wrapping_shl(Y_SHAMT)) & Y_MASK;
    let zn = ((z as u64).wrapping_shl(Z_SHAMT)) & Z_MASK;
    xn | yn | zn
}

pub fn scatter(out: &mut [i64], v: IVec) {
    out[0] = X(v);
    out[1] = Y(v);
    out[2] = Z(v);
}

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

/// +k shifts left, -k right; lanes are sign-extended, so the right shift is arithmetic.
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

/// Drops Z: tiles are keyed by X/Y only, so a point's leaf index must pass
/// through this before it is compared with a tile key. A nonzero Z there
/// dominates a Chebyshev distance and rejects everything.
pub fn TO_XY(i: IVec) -> IVec {
    gather(X(i), Y(i), 0)
}
