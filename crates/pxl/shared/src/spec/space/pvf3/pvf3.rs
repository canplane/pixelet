// pxl-shared/spec/space/pvf3/pvf3.rs

//! PVF3 (Packed 3-Vector w/ Fixed-Point Arithmetic)
//! Fixed-Point Scalar System
//!
//! This module provides atomic, stateless operations for fixed-point arithmetic
//! over PVF3 = [IVec, FVec], where each axis is interpreted as a signed packed
//! fixed-point scalar composed of:
//! - ivec lane: signed integer part
//! - fvec lane: unsigned fractional residue
//!
//! Memory Layout (Per-Axis Fixed-Point Scalar):
//! [ signed integer part | unsigned fractional residue (FRAC_W bits) ]
//!
//! Example Interpretation:
//! - X(v) = ivec.X(v.i) * 2^FRAC_W + fvec.X(v.f)
//! - Y(v) = ivec.Y(v.i) * 2^FRAC_W + fvec.Y(v.f)
//! - Z(v) = ivec.Z(v.i) * 2^FRAC_W + fvec.Z(v.f)
//!
//! Operational Integrity & Overflow Policy:
//! - Deterministic Arithmetic: Uses BigInt for bit-exact fixed-point operations.
//! - Lane Isolation: Runtime storage remains separated into ivec/fvec; arithmetic
//!   is performed on reconstructed fixed-point scalars and re-packed.
//! - Overflow Policy: Final truncation/wrapping is delegated to ivec.gather and
//!   fvec.gather when results are written back into PVF3 storage.
//! - Division Policy: Uses the same bigint integer division/modulo semantics as
//!   current ivec/fvec (i.e. JS bigint `/`, `%`) — truncating, as Rust's.
//!
//! Format Scalability:
//! - Fractional precision is width-parametric via FRAC_W.
//! - Increasing fractional width does not require changes to the arithmetic
//!   surface; only the specification constants change.
//!
//! The module-level scratch register the TypeScript reduces through is a
//! local here, since the reduction owns it for the call.

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
pub struct PVF3 {
    /// 64-bit packed integer address (coarse lattice).
    pub i: IVec,
    /// 32-bit packed fractional remainder (fixed-point lanes).
    pub f: FVec,
}

// ---- Bit Layout Configuration (Fixed-Point Scalar-based) ----

/// Axis Getters: Reconstruct signed fixed-point scalars from ivec/fvec storage.
/// Returned as i64 fixed-point scalars with FRAC_W fractional bits.
pub fn X(v: &PVF3) -> i64 {
    (ivec::X(v.i) << FRAC_W) | fvec::X(v.f)
}
pub fn Y(v: &PVF3) -> i64 {
    (ivec::Y(v.i) << FRAC_W) | fvec::Y(v.f)
}
pub fn Z(v: &PVF3) -> i64 {
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
pub fn repr(v: &PVF3) -> String {
    format!("{{ i: ({}), f: ({}) }}", ivec::repr(v.i), fvec::repr(v.f))
}

// ---- Construction ----

/// Writes the same fixed-point scalar into all three axes.
pub fn fill(out: &mut PVF3, e: i64) -> &mut PVF3 {
    gather(out, e, e, e)
}

/// Gather from fixed-point packed scalars.
///
/// Takes per-axis fixed-point scalars and re-packs them into [IVec, FVec]:
/// - integer part   = arithmetic right shift by FRAC_W
/// - fractional part = low FRAC_W bits
pub fn gather(out: &mut PVF3, x: i64, y: i64, z: i64) -> &mut PVF3 {
    out.i = ivec::gather(x >> FRAC_W, y >> FRAC_W, z >> FRAC_W);
    out.f = fvec::gather(x & ONE_MASK as i64, y & ONE_MASK as i64, z & ONE_MASK as i64);
    out
}

/// Deconstructs a packed vector into its constituent fixed-point scalars.
pub fn scatter(out: &mut [i64], v: &PVF3) {
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
pub fn not<'a>(out: &'a mut PVF3, v: &PVF3) -> &'a mut PVF3 {
    out.i = ivec::not(v.i);
    out.f = fvec::not(v.f);
    out
}
pub fn and<'a>(out: &'a mut PVF3, v: &PVF3, w: &PVF3) -> &'a mut PVF3 {
    out.i = ivec::and(v.i, w.i);
    out.f = fvec::and(v.f, w.f);
    out
}
pub fn or<'a>(out: &'a mut PVF3, v: &PVF3, w: &PVF3) -> &'a mut PVF3 {
    out.i = ivec::or(v.i, w.i);
    out.f = fvec::or(v.f, w.f);
    out
}
pub fn xor<'a>(out: &'a mut PVF3, v: &PVF3, w: &PVF3) -> &'a mut PVF3 {
    out.i = ivec::xor(v.i, w.i);
    out.f = fvec::xor(v.f, w.f);
    out
}

/// Signed shift: +k => shl, -k => shr (arith)
/// - pvf3 axes are SIGNED fixed-point scalars reconstructed by X/Y/Z
/// - right shift is arithmetic (sign-preserving) by construction of bigint >> on signed values
fn shl<'a>(out: &'a mut PVF3, v: &PVF3, shamt: i32) -> &'a mut PVF3 {
    gather(out, X(v).wrapping_shl(shamt as u32), Y(v).wrapping_shl(shamt as u32), Z(v).wrapping_shl(shamt as u32))
}
fn shr<'a>(out: &'a mut PVF3, v: &PVF3, shamt: i32) -> &'a mut PVF3 {
    gather(out, X(v) >> shamt, Y(v) >> shamt, Z(v) >> shamt)
}
pub fn sh<'a>(out: &'a mut PVF3, v: &PVF3, shamt: i32) -> &'a mut PVF3 {
    if shamt >= 0 {
        shl(out, v, shamt)
    } else {
        shr(out, v, -shamt)
    }
}

// ---- Arithmetic ----

/// Arithmetic is defined per-axis on reconstructed fixed-point scalars:
///   op(v, w) = gather(op(X(v), X(w)), op(Y(v), Y(w)), op(Z(v), Z(w)))
pub fn neg<'a>(out: &'a mut PVF3, v: &PVF3) -> &'a mut PVF3 {
    gather(out, X(v).wrapping_neg(), Y(v).wrapping_neg(), Z(v).wrapping_neg())
}
pub fn abs<'a>(out: &'a mut PVF3, v: &PVF3) -> &'a mut PVF3 {
    gather(out, ABS(X(v)), ABS(Y(v)), ABS(Z(v)))
}
pub fn add<'a>(out: &'a mut PVF3, v: &PVF3, w: &PVF3) -> &'a mut PVF3 {
    gather(out, X(v).wrapping_add(X(w)), Y(v).wrapping_add(Y(w)), Z(v).wrapping_add(Z(w)))
}
pub fn sub<'a>(out: &'a mut PVF3, v: &PVF3, w: &PVF3) -> &'a mut PVF3 {
    gather(out, X(v).wrapping_sub(X(w)), Y(v).wrapping_sub(Y(w)), Z(v).wrapping_sub(Z(w)))
}

/// Fixed-point multiply.
/// X/Y/Z already carry FRAC_W fractional bits, so the product is rescaled once by ONE.
pub fn mul<'a>(out: &'a mut PVF3, v: &PVF3, w: &PVF3) -> &'a mut PVF3 {
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
pub fn div<'a>(out: &'a mut PVF3, v: &PVF3, w: &PVF3) -> &'a mut PVF3 {
    gather(
        out,
        (((X(v) as i128) << FRAC_W) / X(w) as i128) as i64,
        (((Y(v) as i128) << FRAC_W) / Y(w) as i128) as i64,
        (((Z(v) as i128) << FRAC_W) / Z(w) as i128) as i64,
    )
}

/// Fixed-point remainder in the packed scalar domain.
pub fn r#mod<'a>(out: &'a mut PVF3, v: &PVF3, w: &PVF3) -> &'a mut PVF3 {
    gather(out, X(v) % X(w), Y(v) % Y(w), Z(v) % Z(w))
}

// ---- Comparison ----

pub fn eq<'a>(out: &'a mut PVF3, v: &PVF3, w: &PVF3) -> &'a mut PVF3 {
    gather(out, EQ(X(v), X(w)), EQ(Y(v), Y(w)), EQ(Z(v), Z(w)))
}
pub fn equals(v: &PVF3, w: &PVF3) -> bool {
    v.i == w.i && v.f == w.f
}
pub fn lt<'a>(out: &'a mut PVF3, v: &PVF3, w: &PVF3) -> &'a mut PVF3 {
    gather(out, LT(X(v), X(w)), LT(Y(v), Y(w)), LT(Z(v), Z(w)))
}
pub fn gt<'a>(out: &'a mut PVF3, v: &PVF3, w: &PVF3) -> &'a mut PVF3 {
    gather(out, GT(X(v), X(w)), GT(Y(v), Y(w)), GT(Z(v), Z(w)))
}
pub fn max_comp(v: &PVF3) -> i64 {
    MAX(MAX(X(v), Y(v)), Z(v))
}
pub fn min_comp(v: &PVF3) -> i64 {
    MIN(MIN(X(v), Y(v)), Z(v))
}
pub fn max<'a>(out: &'a mut PVF3, v: &PVF3, w: &PVF3) -> &'a mut PVF3 {
    gather(out, MAX(X(v), X(w)), MAX(Y(v), Y(w)), MAX(Z(v), Z(w)))
}
pub fn min<'a>(out: &'a mut PVF3, v: &PVF3, w: &PVF3) -> &'a mut PVF3 {
    gather(out, MIN(X(v), X(w)), MIN(Y(v), Y(w)), MIN(Z(v), Z(w)))
}
pub fn clamp<'a>(out: &'a mut PVF3, v: &PVF3, minv: i64, maxv: i64) -> &'a mut PVF3 {
    gather(out, MIN(MAX(X(v), minv), maxv), MIN(MAX(Y(v), minv), maxv), MIN(MAX(Z(v), minv), maxv))
}

// ---- Reduction ----

pub fn sum(v: &PVF3) -> i64 {
    X(v).wrapping_add(Y(v)).wrapping_add(Z(v))
}
pub fn dot(v: &PVF3, w: &PVF3) -> i64 {
    let mut reg = PVF3::default();
    sum(mul(&mut reg, v, w))
}
pub fn norm1(v: &PVF3) -> i64 {
    let mut reg = PVF3::default();
    sum(abs(&mut reg, v))
}
pub fn norm_sq(v: &PVF3) -> i64 {
    dot(v, v)
}
pub fn dist1(v: &PVF3, w: &PVF3) -> i64 {
    let mut reg = PVF3::default();
    norm1(sub(&mut reg, v, w))
}
pub fn dist_sq(v: &PVF3, w: &PVF3) -> i64 {
    let mut reg = PVF3::default();
    norm_sq(sub(&mut reg, v, w))
}
