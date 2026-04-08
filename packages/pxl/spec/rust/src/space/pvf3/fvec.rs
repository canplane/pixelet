// @pxl/spec/space/pvf3/fvec.rs
//
// PVF3 Fractional Residue Precision System.
//
// Memory Layout (32-bit packed):
// [ 31 .. 30 | 29 ... 20 | 19 ... 10 | 9 .... 0 ]
// [ Reserved | Lane X    | Lane Y    | Lane Z   ]
//
// Lanes are unsigned residues in [0, ONE). neg/abs are intentionally omitted.

use super::core::*;

/// FVec: 32-bit packed fractional residue word.
pub type FVec = u64;

// ---- Bit Layout ----

const Z_SHAMT: u32 = 0;
const Y_SHAMT: u32 = FRAC_W;       // 10
const X_SHAMT: u32 = FRAC_W * 2;   // 20

const X_MASK: u64 = field_mask64(FRAC_W, X_SHAMT);
const Y_MASK: u64 = field_mask64(FRAC_W, Y_SHAMT);
const Z_MASK: u64 = field_mask64(FRAC_W, Z_SHAMT);

// ---- Axis Getters (unsigned residues, returned as i64 for surface uniformity) ----

pub fn x(v: FVec) -> i64 { ((v & X_MASK) >> X_SHAMT) as i64 }
pub fn y(v: FVec) -> i64 { ((v & Y_MASK) >> Y_SHAMT) as i64 }
pub fn z(v: FVec) -> i64 { ((v & Z_MASK) >> Z_SHAMT) as i64 }

/// Residue -> normalized real in [0, 1).
pub fn to_real(comp: i64) -> f64 { (comp & ONE_MASK as i64) as f64 / ONE as f64 }

/// Normalized real in [0, 1) -> residue scalar.
pub fn from_real(real: f64) -> i64 {
    let alpha = ((real % 1.0) + 1.0) % 1.0;
    (ONE as f64 * alpha).floor() as i64
}

// ---- Logging ----

pub fn repr(v: FVec) -> String { format!("{}:{}:{}", x(v), y(v), z(v)) }

// ---- Construction ----

pub const fn fill(e: i64) -> FVec { gather(e, e, e) }

pub const fn gather(x: i64, y: i64, z: i64) -> FVec {
    let xn = ((x as u64) << X_SHAMT) & X_MASK;
    let yn = ((y as u64) << Y_SHAMT) & Y_MASK;
    let zn = ((z as u64) << Z_SHAMT) & Z_MASK;
    xn | yn | zn
}

pub fn scatter(v: FVec) -> [i64; 3] { [x(v), y(v), z(v)] }

// ---- Bitwise ----

pub fn not(v: FVec) -> FVec { (!v) & FVEC_WORD_MASK }
pub fn and(v: FVec, w: FVec) -> FVec { v & w }
pub fn or(v: FVec, w: FVec) -> FVec { v | w }
pub fn xor(v: FVec, w: FVec) -> FVec { v ^ w }

/// Signed shift: +shamt => shl, -shamt => logical shr (lanes are unsigned, >> is logical here).
pub fn sh(v: FVec, shamt: i32) -> FVec {
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
// NOTE: neg/abs intentionally omitted — residue domain has no signed negation surface.

pub fn add(v: FVec, w: FVec) -> FVec { gather(x(v).wrapping_add(x(w)), y(v).wrapping_add(y(w)), z(v).wrapping_add(z(w))) }
pub fn sub(v: FVec, w: FVec) -> FVec { gather(x(v).wrapping_sub(x(w)), y(v).wrapping_sub(y(w)), z(v).wrapping_sub(z(w))) }
pub fn mul(v: FVec, w: FVec) -> FVec { gather(x(v).wrapping_mul(x(w)), y(v).wrapping_mul(y(w)), z(v).wrapping_mul(z(w))) }
pub fn div(v: FVec, w: FVec) -> FVec { gather(x(v) / x(w), y(v) / y(w), z(v) / z(w)) }
/// Remainder (TS: `mod`).
pub fn rem(v: FVec, w: FVec) -> FVec { gather(x(v) % x(w), y(v) % y(w), z(v) % z(w)) }

// ---- Comparison ----

pub fn eq(v: FVec, w: FVec) -> FVec { gather(eq_mask(x(v), x(w)), eq_mask(y(v), y(w)), eq_mask(z(v), z(w))) }
pub fn equals(v: FVec, w: FVec) -> bool { v == w }
pub fn lt(v: FVec, w: FVec) -> FVec { gather(lt_mask(x(v), x(w)), lt_mask(y(v), y(w)), lt_mask(z(v), z(w))) }
pub fn gt(v: FVec, w: FVec) -> FVec { gather(gt_mask(x(v), x(w)), gt_mask(y(v), y(w)), gt_mask(z(v), z(w))) }
pub fn max_comp(v: FVec) -> i64 { max_i64(max_i64(x(v), y(v)), z(v)) }
pub fn min_comp(v: FVec) -> i64 { min_i64(min_i64(x(v), y(v)), z(v)) }
pub fn max(v: FVec, w: FVec) -> FVec { gather(max_i64(x(v), x(w)), max_i64(y(v), y(w)), max_i64(z(v), z(w))) }
pub fn min(v: FVec, w: FVec) -> FVec { gather(min_i64(x(v), x(w)), min_i64(y(v), y(w)), min_i64(z(v), z(w))) }
pub fn clamp(v: FVec, lo: i64, hi: i64) -> FVec {
    gather(
        min_i64(max_i64(x(v), lo), hi),
        min_i64(max_i64(y(v), lo), hi),
        min_i64(max_i64(z(v), lo), hi),
    )
}

// ---- Reduction ----

pub fn sum(v: FVec) -> i64 { x(v).wrapping_add(y(v)).wrapping_add(z(v)) }
pub fn dot(v: FVec, w: FVec) -> i64 { sum(mul(v, w)) }
/// Residue-domain proxy: identical to component sum.
pub fn norm1(v: FVec) -> i64 { x(v) + y(v) + z(v) }
pub fn norm_sq(v: FVec) -> i64 { dot(v, v) }
/// Residue-domain proxy over wrapped subtraction.
pub fn dist1(v: FVec, w: FVec) -> i64 { norm1(sub(v, w)) }
pub fn dist_sq(v: FVec, w: FVec) -> i64 { norm_sq(sub(v, w)) }
