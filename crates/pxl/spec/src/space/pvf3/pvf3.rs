// pxl-spec/space/pvf3/pvf3.rs

//! The pair as fixed point: a `Pvf3` is an `IVec` and an `FVec`, and each
//! axis reads as one signed fixed-point scalar,
//!
//!   X(v) = ivec::X(v.i) * 2^FRAC_W + fvec::X(v.f)
//!
//! with the integer part signed and the fraction an unsigned residue. Every
//! operation reconstructs the scalars, computes, and writes the result back
//! through `gather`, which splits it into the two words again; wrapping is
//! `ivec::gather`'s and `fvec::gather`'s. Division and modulo truncate
//! toward zero. The functions write into an `out` the caller owns, and a
//! reduction (`dot`, `norm1`, …) keeps its scratch on its own stack.

// Uppercase throughout, as the notation these operations are written in: one
// that holds no state and closes over nothing is named the way a C header
// names its macros. Rust's own answer to a macro like that is a `const fn`,
// which it cases snake, so the notation and the language disagree here and
// the file keeps the notation — suspended once for the file, which is the
// level the choice is made at rather than item by item.
#![allow(non_snake_case)]

use super::core::{ABS, EQ, FRAC_W, GT, LT, MAX, MIN, ONE, ONE_MASK};
use super::fvec;
use super::fvec::FVec;
use super::ivec;
use super::ivec::IVec;

/// PVF3: Consolidated 96-bit Address Tuple
///
/// Represents a point in a 3D lattice space using:
/// - i : packed signed integer lanes (coarse lattice index)
/// - f : packed unsigned fixed-point lanes (local remainder within a cell)
///
/// Notes:
/// - This type is unitless. Physical meaning (meters, tiles, voxels, etc.)
///   is defined entirely by the caller through scaling outside the kernel.
/// - Default configuration provides fractional resolution of 2^-FRAC_W
///   per axis (e.g. FRAC_W=8 -> 256 steps per lattice unit).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pvf3 {
    /// 64-bit packed integer address (coarse lattice).
    pub i: IVec,
    /// 32-bit packed fractional remainder (fixed-point lanes).
    pub f: FVec,
}

// ---- Bit Layout Configuration (Fixed-Point Scalar-based) ----

/// Axis Getters: Reconstruct signed fixed-point scalars from ivec/fvec storage.
/// Returned as i64 fixed-point scalars with FRAC_W fractional bits.
pub fn X(v: &Pvf3) -> i64 {
    (ivec::X(v.i) << FRAC_W) | fvec::X(v.f)
}
pub fn Y(v: &Pvf3) -> i64 {
    (ivec::Y(v.i) << FRAC_W) | fvec::Y(v.f)
}
pub fn Z(v: &Pvf3) -> i64 {
    (ivec::Z(v.i) << FRAC_W) | fvec::Z(v.f)
}

/// Fixed-point scalar <-> real.
pub fn to_real(comp: i64) -> f64 {
    comp as f64 / ONE as f64
}
pub fn from_real(real: f64) -> i64 {
    (ONE as f64 * real).floor() as i64
}

// ---- Logging ----

/// Human-readable representation of packed fixed-point axis scalars.
/// Purely for logging and debugging; not for runtime logic.
pub fn repr(v: &Pvf3) -> String {
    format!("{{ i: ({}), f: ({}) }}", ivec::repr(v.i), fvec::repr(v.f))
}

// ---- Construction ----

/// Writes the same fixed-point scalar into all three axes.
pub fn fill(out: &mut Pvf3, e: i64) -> &mut Pvf3 {
    gather(out, e, e, e)
}

/// Gather from fixed-point packed scalars.
///
/// Takes per-axis fixed-point scalars and re-packs them into [IVec, FVec]:
/// - integer part   = arithmetic right shift by FRAC_W
/// - fractional part = low FRAC_W bits
pub fn gather(out: &mut Pvf3, x: i64, y: i64, z: i64) -> &mut Pvf3 {
    out.i = ivec::gather(x >> FRAC_W, y >> FRAC_W, z >> FRAC_W);
    out.f = fvec::gather(x & ONE_MASK as i64, y & ONE_MASK as i64, z & ONE_MASK as i64);
    out
}

/// Deconstructs a packed vector into its constituent fixed-point scalars.
pub fn scatter(out: &mut [i64], v: &Pvf3) {
    out[0] = X(v);
    out[1] = Y(v);
    out[2] = Z(v);
}

// ---- Bitwise ----

/// Reuse raw storage-domain bitwise ops directly.
///
/// This is the closest extension of ivec/fvec behavior:
/// - ivec bitwise on integer part
/// - fvec bitwise on fractional part
pub fn not<'a>(out: &'a mut Pvf3, v: &Pvf3) -> &'a mut Pvf3 {
    out.i = ivec::not(v.i);
    out.f = fvec::not(v.f);
    out
}
pub fn and<'a>(out: &'a mut Pvf3, v: &Pvf3, w: &Pvf3) -> &'a mut Pvf3 {
    out.i = ivec::and(v.i, w.i);
    out.f = fvec::and(v.f, w.f);
    out
}
pub fn or<'a>(out: &'a mut Pvf3, v: &Pvf3, w: &Pvf3) -> &'a mut Pvf3 {
    out.i = ivec::or(v.i, w.i);
    out.f = fvec::or(v.f, w.f);
    out
}
pub fn xor<'a>(out: &'a mut Pvf3, v: &Pvf3, w: &Pvf3) -> &'a mut Pvf3 {
    out.i = ivec::xor(v.i, w.i);
    out.f = fvec::xor(v.f, w.f);
    out
}

/// Signed shift: +k => shl, -k => shr. An axis is a signed fixed-point
/// scalar as X/Y/Z reconstruct it, so the right shift is arithmetic.
fn shl<'a>(out: &'a mut Pvf3, v: &Pvf3, shamt: i32) -> &'a mut Pvf3 {
    gather(out, X(v).wrapping_shl(shamt as u32), Y(v).wrapping_shl(shamt as u32), Z(v).wrapping_shl(shamt as u32))
}
fn shr<'a>(out: &'a mut Pvf3, v: &Pvf3, shamt: i32) -> &'a mut Pvf3 {
    gather(out, X(v) >> shamt, Y(v) >> shamt, Z(v) >> shamt)
}
pub fn sh<'a>(out: &'a mut Pvf3, v: &Pvf3, shamt: i32) -> &'a mut Pvf3 {
    if shamt >= 0 {
        shl(out, v, shamt)
    } else {
        shr(out, v, -shamt)
    }
}

// ---- Arithmetic ----

/// Arithmetic is defined per-axis on reconstructed fixed-point scalars:
///   op(v, w) = gather(op(X(v), X(w)), op(Y(v), Y(w)), op(Z(v), Z(w)))
pub fn neg<'a>(out: &'a mut Pvf3, v: &Pvf3) -> &'a mut Pvf3 {
    gather(out, X(v).wrapping_neg(), Y(v).wrapping_neg(), Z(v).wrapping_neg())
}
pub fn abs<'a>(out: &'a mut Pvf3, v: &Pvf3) -> &'a mut Pvf3 {
    gather(out, ABS(X(v)), ABS(Y(v)), ABS(Z(v)))
}
pub fn add<'a>(out: &'a mut Pvf3, v: &Pvf3, w: &Pvf3) -> &'a mut Pvf3 {
    gather(out, X(v).wrapping_add(X(w)), Y(v).wrapping_add(Y(w)), Z(v).wrapping_add(Z(w)))
}
pub fn sub<'a>(out: &'a mut Pvf3, v: &Pvf3, w: &Pvf3) -> &'a mut Pvf3 {
    gather(out, X(v).wrapping_sub(X(w)), Y(v).wrapping_sub(Y(w)), Z(v).wrapping_sub(Z(w)))
}

/// Fixed-point multiply.
/// X/Y/Z already carry FRAC_W fractional bits, so the product is rescaled once by ONE.
pub fn mul<'a>(out: &'a mut Pvf3, v: &Pvf3, w: &Pvf3) -> &'a mut Pvf3 {
    let one = ONE as i128;
    gather(
        out,
        ((X(v) as i128 * X(w) as i128) / one) as i64,
        ((Y(v) as i128 * Y(w) as i128) / one) as i64,
        ((Z(v) as i128 * Z(w) as i128) / one) as i64,
    )
}

/// Fixed-point divide.
/// To preserve FRAC_W fractional bits, the numerator is pre-scaled by ONE.
pub fn div<'a>(out: &'a mut Pvf3, v: &Pvf3, w: &Pvf3) -> &'a mut Pvf3 {
    gather(
        out,
        (((X(v) as i128) << FRAC_W) / X(w) as i128) as i64,
        (((Y(v) as i128) << FRAC_W) / Y(w) as i128) as i64,
        (((Z(v) as i128) << FRAC_W) / Z(w) as i128) as i64,
    )
}

/// Fixed-point remainder in the packed scalar domain.
pub fn r#mod<'a>(out: &'a mut Pvf3, v: &Pvf3, w: &Pvf3) -> &'a mut Pvf3 {
    gather(out, X(v) % X(w), Y(v) % Y(w), Z(v) % Z(w))
}

// ---- Comparison ----

pub fn eq<'a>(out: &'a mut Pvf3, v: &Pvf3, w: &Pvf3) -> &'a mut Pvf3 {
    gather(out, EQ(X(v), X(w)), EQ(Y(v), Y(w)), EQ(Z(v), Z(w)))
}
pub fn equals(v: &Pvf3, w: &Pvf3) -> bool {
    v.i == w.i && v.f == w.f
}
pub fn lt<'a>(out: &'a mut Pvf3, v: &Pvf3, w: &Pvf3) -> &'a mut Pvf3 {
    gather(out, LT(X(v), X(w)), LT(Y(v), Y(w)), LT(Z(v), Z(w)))
}
pub fn gt<'a>(out: &'a mut Pvf3, v: &Pvf3, w: &Pvf3) -> &'a mut Pvf3 {
    gather(out, GT(X(v), X(w)), GT(Y(v), Y(w)), GT(Z(v), Z(w)))
}
pub fn max_comp(v: &Pvf3) -> i64 {
    MAX(MAX(X(v), Y(v)), Z(v))
}
pub fn min_comp(v: &Pvf3) -> i64 {
    MIN(MIN(X(v), Y(v)), Z(v))
}
pub fn max<'a>(out: &'a mut Pvf3, v: &Pvf3, w: &Pvf3) -> &'a mut Pvf3 {
    gather(out, MAX(X(v), X(w)), MAX(Y(v), Y(w)), MAX(Z(v), Z(w)))
}
pub fn min<'a>(out: &'a mut Pvf3, v: &Pvf3, w: &Pvf3) -> &'a mut Pvf3 {
    gather(out, MIN(X(v), X(w)), MIN(Y(v), Y(w)), MIN(Z(v), Z(w)))
}
pub fn clamp<'a>(out: &'a mut Pvf3, v: &Pvf3, minv: i64, maxv: i64) -> &'a mut Pvf3 {
    gather(out, MIN(MAX(X(v), minv), maxv), MIN(MAX(Y(v), minv), maxv), MIN(MAX(Z(v), minv), maxv))
}

// ---- Reduction ----

pub fn sum(v: &Pvf3) -> i64 {
    X(v).wrapping_add(Y(v)).wrapping_add(Z(v))
}
pub fn dot(v: &Pvf3, w: &Pvf3) -> i64 {
    let mut reg = Pvf3::default();
    sum(mul(&mut reg, v, w))
}
pub fn norm1(v: &Pvf3) -> i64 {
    let mut reg = Pvf3::default();
    sum(abs(&mut reg, v))
}
pub fn norm_sq(v: &Pvf3) -> i64 {
    dot(v, v)
}
pub fn dist1(v: &Pvf3, w: &Pvf3) -> i64 {
    let mut reg = Pvf3::default();
    norm1(sub(&mut reg, v, w))
}
pub fn dist_sq(v: &Pvf3, w: &Pvf3) -> i64 {
    let mut reg = Pvf3::default();
    norm_sq(sub(&mut reg, v, w))
}
