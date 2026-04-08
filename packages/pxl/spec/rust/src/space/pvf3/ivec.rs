// @pxl/spec/space/pvf3/ivec.rs
//
// PVF3 Integer Component Precision System.
//
// Memory Layout (64-bit packed):
// [ 63 ............ 40 | 39 ............ 16 | 15 ........... 0 ]
// [   Lane X (24 bits)  |   Lane Y (24 bits)  |  Lane Z (16 bits) ]
//
// Lanes are signed two's-complement. Arithmetic is per-lane with modular wrapping.

use super::core::*;

/// IVec: 64-bit packed integer address (coarse lattice index).
pub type IVec = u64;

// ---- Bit Layout ----

const Z_SHAMT: u32 = 0;
const Y_SHAMT: u32 = IVEC_Z_W;              // 16
const X_SHAMT: u32 = IVEC_Z_W + IVEC_Y_W;  // 40

pub const X_MASK: u64 = field_mask64(IVEC_X_W, X_SHAMT);
pub const Y_MASK: u64 = field_mask64(IVEC_Y_W, Y_SHAMT);
pub const Z_MASK: u64 = field_mask64(IVEC_Z_W, Z_SHAMT);

// ---- Axis Getters (signed) ----

#[inline]
fn sign_ext(raw: u64, w: u32) -> i64 {
    let shift = 64 - w;
    (raw as i64).wrapping_shl(shift).wrapping_shr(shift)
}

pub fn x(v: IVec) -> i64 { sign_ext(v >> X_SHAMT, IVEC_X_W) }
pub fn y(v: IVec) -> i64 { sign_ext(v >> Y_SHAMT, IVEC_Y_W) }
pub fn z(v: IVec) -> i64 { sign_ext(v >> Z_SHAMT, IVEC_Z_W) }

pub fn to_real(comp: i64) -> f64 { comp as f64 }
pub fn from_real(real: f64) -> i64 { real.floor() as i64 }

// ---- Logging ----

pub fn repr(v: IVec) -> String { format!("{}:{}:{}", x(v), y(v), z(v)) }

// ---- Construction ----

pub const fn fill(e: i64) -> IVec { gather(e, e, e) }

pub const fn gather(x: i64, y: i64, z: i64) -> IVec {
    let xn = ((x as u64) << X_SHAMT) & X_MASK;
    let yn = ((y as u64) << Y_SHAMT) & Y_MASK;
    let zn = ((z as u64) << Z_SHAMT) & Z_MASK;
    xn | yn | zn
}

pub fn scatter(v: IVec) -> [i64; 3] { [x(v), y(v), z(v)] }

// ---- Bitwise ----

pub fn not(v: IVec) -> IVec { (!v) & IVEC_WORD_MASK }
pub fn and(v: IVec, w: IVec) -> IVec { v & w }
pub fn or(v: IVec, w: IVec) -> IVec { v | w }
pub fn xor(v: IVec, w: IVec) -> IVec { v ^ w }

/// Signed shift: +shamt => shl, -shamt => arithmetic shr.
pub fn sh(v: IVec, shamt: i32) -> IVec {
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

pub fn neg(v: IVec) -> IVec { gather(x(v).wrapping_neg(), y(v).wrapping_neg(), z(v).wrapping_neg()) }
pub fn abs(v: IVec) -> IVec { gather(abs_i64(x(v)), abs_i64(y(v)), abs_i64(z(v))) }

pub fn add(v: IVec, w: IVec) -> IVec { gather(x(v).wrapping_add(x(w)), y(v).wrapping_add(y(w)), z(v).wrapping_add(z(w))) }
pub fn sub(v: IVec, w: IVec) -> IVec { gather(x(v).wrapping_sub(x(w)), y(v).wrapping_sub(y(w)), z(v).wrapping_sub(z(w))) }
pub fn mul(v: IVec, w: IVec) -> IVec { gather(x(v).wrapping_mul(x(w)), y(v).wrapping_mul(y(w)), z(v).wrapping_mul(z(w))) }
pub fn div(v: IVec, w: IVec) -> IVec { gather(x(v) / x(w), y(v) / y(w), z(v) / z(w)) }
/// Remainder (TS: `mod`). Sign follows dividend.
pub fn rem(v: IVec, w: IVec) -> IVec { gather(x(v) % x(w), y(v) % y(w), z(v) % z(w)) }

// ---- Comparison ----

pub fn eq(v: IVec, w: IVec) -> IVec { gather(eq_mask(x(v), x(w)), eq_mask(y(v), y(w)), eq_mask(z(v), z(w))) }
pub fn equals(v: IVec, w: IVec) -> bool { v == w }
pub fn lt(v: IVec, w: IVec) -> IVec { gather(lt_mask(x(v), x(w)), lt_mask(y(v), y(w)), lt_mask(z(v), z(w))) }
pub fn gt(v: IVec, w: IVec) -> IVec { gather(gt_mask(x(v), x(w)), gt_mask(y(v), y(w)), gt_mask(z(v), z(w))) }
pub fn max_comp(v: IVec) -> i64 { max_i64(max_i64(x(v), y(v)), z(v)) }
pub fn min_comp(v: IVec) -> i64 { min_i64(min_i64(x(v), y(v)), z(v)) }
pub fn max(v: IVec, w: IVec) -> IVec { gather(max_i64(x(v), x(w)), max_i64(y(v), y(w)), max_i64(z(v), z(w))) }
pub fn min(v: IVec, w: IVec) -> IVec { gather(min_i64(x(v), x(w)), min_i64(y(v), y(w)), min_i64(z(v), z(w))) }
pub fn clamp(v: IVec, lo: i64, hi: i64) -> IVec {
    gather(
        min_i64(max_i64(x(v), lo), hi),
        min_i64(max_i64(y(v), lo), hi),
        min_i64(max_i64(z(v), lo), hi),
    )
}

// ---- Reduction ----

pub fn sum(v: IVec) -> i64 { x(v).wrapping_add(y(v)).wrapping_add(z(v)) }
pub fn dot(v: IVec, w: IVec) -> i64 { sum(mul(v, w)) }
pub fn norm1(v: IVec) -> i64 { sum(abs(v)) }
pub fn norm_sq(v: IVec) -> i64 { dot(v, v) }
pub fn dist1(v: IVec, w: IVec) -> i64 { norm1(sub(v, w)) }
pub fn dist_sq(v: IVec, w: IVec) -> i64 { norm_sq(sub(v, w)) }

// ============================================================
// Pointer-semantic extensions
// ============================================================
//
// NOTE: IVec is a spatial *address* (coarse lattice pointer), not a free vector.
// The arithmetic surface above (mul/div/neg/abs/dot/norm/dist) is preserved for
// compatibility with the TS port, but most of those operations are semantically
// undefined on absolute coordinate values. The functions below are the intended
// API for displacement math.
//
// TODO(guard-bits): The current 24-24-16 layout has no guard bits (see core.rs).
// As a result, every add/sub/sh must unpack each lane, operate, and repack —
// ~3× the cost of direct packed arithmetic. Once the layout gains guard bits,
// offset/translate can operate on the packed u64 word directly.
//
// TODO(overflow): offset() currently uses wrapping_sub, which silently discards
// whether the X lane wrapped across the antimeridian seam (cyclic boundary).
// A future version should return overflow flags alongside the delta so callers
// can choose the shortest-path direction across the seam.

/// Signed lattice-unit displacement from `from` to `to`, per lane, unpacked.
/// Intended for direct f64 conversion or further arithmetic — does not repack.
pub fn offset(from: IVec, to: IVec) -> [i64; 3] {
    [
        x(to).wrapping_sub(x(from)),
        y(to).wrapping_sub(y(from)),
        z(to).wrapping_sub(z(from)),
    ]
}

/// Applies a per-lane lattice-unit delta to an IVec address.
pub fn translate(p: IVec, d: [i64; 3]) -> IVec {
    gather(
        x(p).wrapping_add(d[0]),
        y(p).wrapping_add(d[1]),
        z(p).wrapping_add(d[2]),
    )
}
