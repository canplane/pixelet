// pxl-spec/space/coord/pvf3/pvf3.rs

//! A `Pvf3` is an `IVec` and an `FVec`; each axis reads as one signed fixed-point scalar,
//!
//!   X(v) = ivec::X(v.i) * 2^FRAC_W + fvec::X(v.f)
//!
//! Operations reconstruct the scalars, compute, and repack through `gather`;
//! each returns its result, as `ivec`'s and `fvec`'s do.
//! Division and modulo truncate toward zero. A point and a displacement share
//! the form, but only a displacement adds to a point: two points summed name
//! no place.

// Uppercase as the notation these macro-like helpers are written in.
#![allow(non_snake_case)]

use super::core::{ABS, EQ, FRAC_W, GT, LT, MAX, MIN, ONE, ONE_MASK};
use super::fvec;
use super::fvec::FVec;
use super::ivec;
use super::ivec::IVec;

/// A unitless point: packed signed integer lanes and a packed unsigned
/// fraction of 2^-FRAC_W per axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pvf3 {
    pub i: IVec,
    pub f: FVec,
}

/// Signed fixed-point scalars with FRAC_W fractional bits.
pub fn X(v: &Pvf3) -> i64 {
    (ivec::X(v.i) << FRAC_W) | fvec::X(v.f)
}
pub fn Y(v: &Pvf3) -> i64 {
    (ivec::Y(v.i) << FRAC_W) | fvec::Y(v.f)
}
pub fn Z(v: &Pvf3) -> i64 {
    (ivec::Z(v.i) << FRAC_W) | fvec::Z(v.f)
}

pub fn to_real(comp: i64) -> f64 {
    comp as f64 / ONE as f64
}
pub fn from_real(real: f64) -> i64 {
    (ONE as f64 * real).floor() as i64
}

/// For logging only.
pub fn repr(v: &Pvf3) -> String {
    format!("{{ i: ({}), f: ({}) }}", ivec::repr(v.i), fvec::repr(v.f))
}

pub fn fill(e: i64) -> Pvf3 {
    gather(e, e, e)
}

/// Repack per-axis fixed-point scalars into the two words.
pub fn gather(x: i64, y: i64, z: i64) -> Pvf3 {
    Pvf3 {
        i: ivec::gather(x >> FRAC_W, y >> FRAC_W, z >> FRAC_W),
        f: fvec::gather(x & ONE_MASK as i64, y & ONE_MASK as i64, z & ONE_MASK as i64),
    }
}

pub fn scatter(out: &mut [i64], v: &Pvf3) {
    out[0] = X(v);
    out[1] = Y(v);
    out[2] = Z(v);
}

pub fn not(v: &Pvf3) -> Pvf3 {
    Pvf3 { i: ivec::not(v.i), f: fvec::not(v.f) }
}
pub fn and(v: &Pvf3, w: &Pvf3) -> Pvf3 {
    Pvf3 {
        i: ivec::and(v.i, w.i),
        f: fvec::and(v.f, w.f),
    }
}
pub fn or(v: &Pvf3, w: &Pvf3) -> Pvf3 {
    Pvf3 {
        i: ivec::or(v.i, w.i),
        f: fvec::or(v.f, w.f),
    }
}
pub fn xor(v: &Pvf3, w: &Pvf3) -> Pvf3 {
    Pvf3 {
        i: ivec::xor(v.i, w.i),
        f: fvec::xor(v.f, w.f),
    }
}

/// +k shifts left, -k right; the right shift is arithmetic.
fn shl(v: &Pvf3, shamt: i32) -> Pvf3 {
    gather(X(v).wrapping_shl(shamt as u32), Y(v).wrapping_shl(shamt as u32), Z(v).wrapping_shl(shamt as u32))
}
fn shr(v: &Pvf3, shamt: i32) -> Pvf3 {
    gather(X(v) >> shamt, Y(v) >> shamt, Z(v) >> shamt)
}
pub fn sh(v: &Pvf3, shamt: i32) -> Pvf3 {
    if shamt >= 0 {
        shl(v, shamt)
    } else {
        shr(v, -shamt)
    }
}

pub fn neg(v: &Pvf3) -> Pvf3 {
    gather(X(v).wrapping_neg(), Y(v).wrapping_neg(), Z(v).wrapping_neg())
}
pub fn abs(v: &Pvf3) -> Pvf3 {
    gather(ABS(X(v)), ABS(Y(v)), ABS(Z(v)))
}
pub fn add(v: &Pvf3, w: &Pvf3) -> Pvf3 {
    gather(X(v).wrapping_add(X(w)), Y(v).wrapping_add(Y(w)), Z(v).wrapping_add(Z(w)))
}
pub fn sub(v: &Pvf3, w: &Pvf3) -> Pvf3 {
    gather(X(v).wrapping_sub(X(w)), Y(v).wrapping_sub(Y(w)), Z(v).wrapping_sub(Z(w)))
}

/// The product is rescaled once by ONE.
pub fn mul(v: &Pvf3, w: &Pvf3) -> Pvf3 {
    let one = ONE as i128;
    gather(
        ((X(v) as i128 * X(w) as i128) / one) as i64,
        ((Y(v) as i128 * Y(w) as i128) / one) as i64,
        ((Z(v) as i128 * Z(w) as i128) / one) as i64,
    )
}

/// The numerator is pre-scaled by ONE to keep the fraction.
pub fn div(v: &Pvf3, w: &Pvf3) -> Pvf3 {
    gather(
        (((X(v) as i128) << FRAC_W) / X(w) as i128) as i64,
        (((Y(v) as i128) << FRAC_W) / Y(w) as i128) as i64,
        (((Z(v) as i128) << FRAC_W) / Z(w) as i128) as i64,
    )
}

pub fn r#mod(v: &Pvf3, w: &Pvf3) -> Pvf3 {
    gather(X(v) % X(w), Y(v) % Y(w), Z(v) % Z(w))
}

pub fn eq(v: &Pvf3, w: &Pvf3) -> Pvf3 {
    gather(EQ(X(v), X(w)), EQ(Y(v), Y(w)), EQ(Z(v), Z(w)))
}
pub fn equals(v: &Pvf3, w: &Pvf3) -> bool {
    v.i == w.i && v.f == w.f
}
pub fn lt(v: &Pvf3, w: &Pvf3) -> Pvf3 {
    gather(LT(X(v), X(w)), LT(Y(v), Y(w)), LT(Z(v), Z(w)))
}
pub fn gt(v: &Pvf3, w: &Pvf3) -> Pvf3 {
    gather(GT(X(v), X(w)), GT(Y(v), Y(w)), GT(Z(v), Z(w)))
}
pub fn max_comp(v: &Pvf3) -> i64 {
    MAX(MAX(X(v), Y(v)), Z(v))
}
pub fn min_comp(v: &Pvf3) -> i64 {
    MIN(MIN(X(v), Y(v)), Z(v))
}
pub fn max(v: &Pvf3, w: &Pvf3) -> Pvf3 {
    gather(MAX(X(v), X(w)), MAX(Y(v), Y(w)), MAX(Z(v), Z(w)))
}
pub fn min(v: &Pvf3, w: &Pvf3) -> Pvf3 {
    gather(MIN(X(v), X(w)), MIN(Y(v), Y(w)), MIN(Z(v), Z(w)))
}
pub fn clamp(v: &Pvf3, minv: i64, maxv: i64) -> Pvf3 {
    gather(MIN(MAX(X(v), minv), maxv), MIN(MAX(Y(v), minv), maxv), MIN(MAX(Z(v), minv), maxv))
}

pub fn sum(v: &Pvf3) -> i64 {
    X(v).wrapping_add(Y(v)).wrapping_add(Z(v))
}
pub fn dot(v: &Pvf3, w: &Pvf3) -> i64 {
    sum(&mul(v, w))
}
pub fn norm1(v: &Pvf3) -> i64 {
    sum(&abs(v))
}
pub fn norm_sq(v: &Pvf3) -> i64 {
    dot(v, v)
}
pub fn dist1(v: &Pvf3, w: &Pvf3) -> i64 {
    norm1(&sub(v, w))
}
pub fn dist_sq(v: &Pvf3, w: &Pvf3) -> i64 {
    norm_sq(&sub(v, w))
}
