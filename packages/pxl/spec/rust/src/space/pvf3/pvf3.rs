// @pxl/spec/space/pvf3/pvf3.rs
//
// PVF3 Fixed-Point Scalar System.
//
// PVF3 = { i: IVec, f: FVec }
//
// Per-axis fixed-point scalar:
//   X(v) = ivec::x(v.i) * ONE + fvec::x(v.f)
//        = (ivec::x(v.i) << FRAC_W) | fvec::x(v.f)
//
// Arithmetic is performed on reconstructed fixed-point scalars and re-packed via gather.
// Unlike ivec/fvec, pvf3 functions take/return PVF3 by value (no out-parameter).

use super::core::*;
use super::{fvec, fvec::FVec, ivec, ivec::IVec};

/// PVF3: Consolidated fixed-point 3-vector (96 bits: 64-bit IVec + 32-bit FVec).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pvf3 {
    pub i: IVec,
    pub f: FVec,
}

impl Pvf3 {
    pub const ZERO: Pvf3 = Pvf3 { i: 0, f: 0 };
}

// ---- Axis Getters (signed fixed-point scalars with FRAC_W fractional bits) ----

pub fn x(v: Pvf3) -> i64 { (ivec::x(v.i) << FRAC_W as i64) | fvec::x(v.f) }
pub fn y(v: Pvf3) -> i64 { (ivec::y(v.i) << FRAC_W as i64) | fvec::y(v.f) }
pub fn z(v: Pvf3) -> i64 { (ivec::z(v.i) << FRAC_W as i64) | fvec::z(v.f) }

pub fn to_real(comp: i64) -> f64 { comp as f64 / ONE as f64 }
pub fn from_real(real: f64) -> i64 { (ONE as f64 * real).floor() as i64 }

// ---- Logging ----

pub fn repr(v: Pvf3) -> String {
    format!("{{ i: ({}), f: ({}) }}", ivec::repr(v.i), fvec::repr(v.f))
}

// ---- Construction ----

pub fn fill(e: i64) -> Pvf3 { gather(e, e, e) }

/// Gather from fixed-point scalars: splits each into integer (>> FRAC_W) and
/// fractional (& ONE_MASK) parts and packs into PVF3.
pub fn gather(x: i64, y: i64, z: i64) -> Pvf3 {
    Pvf3 {
        i: ivec::gather(x >> FRAC_W as i64, y >> FRAC_W as i64, z >> FRAC_W as i64),
        f: fvec::gather(x & ONE_MASK as i64, y & ONE_MASK as i64, z & ONE_MASK as i64),
    }
}

pub fn scatter(v: Pvf3) -> [i64; 3] { [x(v), y(v), z(v)] }

// ---- Bitwise ----

pub fn not(v: Pvf3) -> Pvf3 { Pvf3 { i: ivec::not(v.i), f: fvec::not(v.f) } }
pub fn and(v: Pvf3, w: Pvf3) -> Pvf3 { Pvf3 { i: ivec::and(v.i, w.i), f: fvec::and(v.f, w.f) } }
pub fn or(v: Pvf3, w: Pvf3) -> Pvf3 { Pvf3 { i: ivec::or(v.i, w.i), f: fvec::or(v.f, w.f) } }
pub fn xor(v: Pvf3, w: Pvf3) -> Pvf3 { Pvf3 { i: ivec::xor(v.i, w.i), f: fvec::xor(v.f, w.f) } }

/// Signed shift on fixed-point scalars.
pub fn sh(v: Pvf3, shamt: i32) -> Pvf3 {
    if shamt >= 0 {
        let s = shamt as u32;
        gather(
            x(v).wrapping_shl(s),
            y(v).wrapping_shl(s),
            z(v).wrapping_shl(s),
        )
    } else {
        let s = shamt.unsigned_abs();
        gather(
            x(v).wrapping_shr(s),
            y(v).wrapping_shr(s),
            z(v).wrapping_shr(s),
        )
    }
}

// ---- Arithmetic ----

pub fn neg(v: Pvf3) -> Pvf3 { gather(x(v).wrapping_neg(), y(v).wrapping_neg(), z(v).wrapping_neg()) }
pub fn abs(v: Pvf3) -> Pvf3 { gather(abs_i64(x(v)), abs_i64(y(v)), abs_i64(z(v))) }
pub fn add(v: Pvf3, w: Pvf3) -> Pvf3 { gather(x(v).wrapping_add(x(w)), y(v).wrapping_add(y(w)), z(v).wrapping_add(z(w))) }
pub fn sub(v: Pvf3, w: Pvf3) -> Pvf3 { gather(x(v).wrapping_sub(x(w)), y(v).wrapping_sub(y(w)), z(v).wrapping_sub(z(w))) }

/// Fixed-point multiply: rescale product by ONE to maintain FRAC_W fractional bits.
pub fn mul(v: Pvf3, w: Pvf3) -> Pvf3 {
    gather(
        x(v).wrapping_mul(x(w)) / ONE as i64,
        y(v).wrapping_mul(y(w)) / ONE as i64,
        z(v).wrapping_mul(z(w)) / ONE as i64,
    )
}

/// Fixed-point divide: pre-scale numerator by ONE to preserve FRAC_W fractional bits.
pub fn div(v: Pvf3, w: Pvf3) -> Pvf3 {
    gather(
        (x(v).wrapping_shl(FRAC_W)) / x(w),
        (y(v).wrapping_shl(FRAC_W)) / y(w),
        (z(v).wrapping_shl(FRAC_W)) / z(w),
    )
}

/// Remainder (TS: `mod`).
pub fn rem(v: Pvf3, w: Pvf3) -> Pvf3 { gather(x(v) % x(w), y(v) % y(w), z(v) % z(w)) }

// ---- Comparison ----

pub fn eq(v: Pvf3, w: Pvf3) -> Pvf3 { gather(eq_mask(x(v), x(w)), eq_mask(y(v), y(w)), eq_mask(z(v), z(w))) }
pub fn equals(v: Pvf3, w: Pvf3) -> bool { v.i == w.i && v.f == w.f }
pub fn lt(v: Pvf3, w: Pvf3) -> Pvf3 { gather(lt_mask(x(v), x(w)), lt_mask(y(v), y(w)), lt_mask(z(v), z(w))) }
pub fn gt(v: Pvf3, w: Pvf3) -> Pvf3 { gather(gt_mask(x(v), x(w)), gt_mask(y(v), y(w)), gt_mask(z(v), z(w))) }
pub fn max_comp(v: Pvf3) -> i64 { max_i64(max_i64(x(v), y(v)), z(v)) }
pub fn min_comp(v: Pvf3) -> i64 { min_i64(min_i64(x(v), y(v)), z(v)) }
pub fn max(v: Pvf3, w: Pvf3) -> Pvf3 { gather(max_i64(x(v), x(w)), max_i64(y(v), y(w)), max_i64(z(v), z(w))) }
pub fn min(v: Pvf3, w: Pvf3) -> Pvf3 { gather(min_i64(x(v), x(w)), min_i64(y(v), y(w)), min_i64(z(v), z(w))) }
pub fn clamp(v: Pvf3, lo: i64, hi: i64) -> Pvf3 {
    gather(
        min_i64(max_i64(x(v), lo), hi),
        min_i64(max_i64(y(v), lo), hi),
        min_i64(max_i64(z(v), lo), hi),
    )
}

// ---- Reduction ----

pub fn sum(v: Pvf3) -> i64 { x(v).wrapping_add(y(v)).wrapping_add(z(v)) }
pub fn dot(v: Pvf3, w: Pvf3) -> i64 { sum(mul(v, w)) }
pub fn norm1(v: Pvf3) -> i64 { sum(abs(v)) }
pub fn norm_sq(v: Pvf3) -> i64 { dot(v, v) }
pub fn dist1(v: Pvf3, w: Pvf3) -> i64 { norm1(sub(v, w)) }
pub fn dist_sq(v: Pvf3, w: Pvf3) -> i64 { norm_sq(sub(v, w)) }

// ============================================================
// Pointer-semantic extensions
// ============================================================
//
// NOTE: PVF3 is a 96-bit spatial *address* (pointer into the lattice), not a
// free vector. The arithmetic surface above (mul/div/neg/abs/dot/norm/dist) is
// preserved for compatibility with the TS port but has no semantic meaning for
// absolute coordinate values — you don't multiply two map addresses.
//
// The correct mental model:
//   PVF3           = pointer   ("where am I in the lattice?")
//   [f64; 3]       = offset    ("how far do I move?")
//   offset(a, b)   = b - a     (pointer difference → world-space displacement)
//   translate(p,d) = p + d     (pointer advance by world-space delta)
//
// TODO(guard-bits): offset/translate currently round-trip through
// projection::unproject + project (two floor() decompositions per call).
// Once IVec gains guard bits (see core.rs), small integer displacements can
// bypass the float round-trip entirely: translate by an integer delta stays in
// the integer domain and only the FVec residue needs updating.
//
// TODO(overflow): offset() across the antimeridian seam (X-lane cyclic
// boundary) returns a value that may silently wrap. Callers that need the
// shortest-path delta should check whether |offset[0]| > half the X range and
// correct accordingly. A guard-bit carry flag would make this automatic.

/// World-space displacement from `from` to `to`, in normalized lattice units (`u`).
///
/// Result is a plain [f64; 3] — not a PVF3 — because displacement is a free vector,
/// not an absolute address.
pub fn offset(from: Pvf3, to: Pvf3) -> [f64; 3] {
    let [fx, fy, fz] = unproject(from);
    let [tx, ty, tz] = unproject(to);
    [tx - fx, ty - fy, tz - fz]
}

/// Applies a world-space delta (in normalized lattice units) to a PVF3 address.
pub fn translate(p: Pvf3, delta: [f64; 3]) -> Pvf3 {
    let [px, py, pz] = unproject(p);
    project(px + delta[0], py + delta[1], pz + delta[2])
}

// Inline projection helpers (mirrors projection.rs; kept local to avoid the circular
// dependency: projection.rs imports pvf3::PVF3, so pvf3.rs cannot import projection).
fn unproject(p: Pvf3) -> [f64; 3] {
    [
        ivec::to_real(ivec::x(p.i)) + fvec::to_real(fvec::x(p.f)),
        ivec::to_real(ivec::y(p.i)) + fvec::to_real(fvec::y(p.f)),
        ivec::to_real(ivec::z(p.i)) + fvec::to_real(fvec::z(p.f)),
    ]
}
fn project(x: f64, y: f64, z: f64) -> Pvf3 {
    Pvf3 {
        i: ivec::gather(ivec::from_real(x), ivec::from_real(y), ivec::from_real(z)),
        f: fvec::gather(fvec::from_real(x), fvec::from_real(y), fvec::from_real(z)),
    }
}
