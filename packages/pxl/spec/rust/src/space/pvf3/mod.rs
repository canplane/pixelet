// @pxl/spec/space/pvf3/mod.rs
//
// PVF3: Packed 3-Vector with Fixed-Point Arithmetic.
//
// Modules:
//   core       — bit-width constants, field mask utilities, atomic ops (ABS/EQ/LT/GT/MAX/MIN)
//   ivec       — signed integer lanes (24-24-16 bit asymmetric)
//   fvec       — unsigned fractional residue lanes (10-10-10 bit symmetric)
//   pvf3       — fixed-point tuple (IVec + FVec), return-by-value arithmetic
//   projection — project/unproject between f64 and PVF3

pub mod core;
pub mod fvec;
pub mod ivec;
pub mod projection;
#[allow(clippy::module_inception)]
pub mod pvf3;

pub use fvec::FVec;
pub use ivec::IVec;
pub use pvf3::Pvf3;

// ---- PVecTrait ----
// Common operation contract for packed-word vector modules (ivec and fvec).
// In Rust, modules cannot implement traits directly; this is provided for
// documentation purposes. Wrap in a struct to get a concrete impl if needed.
//
// NOTE: `pvf3` is intentionally excluded — it is a tuple family with distinct arithmetic.

pub trait PVecTrait<V: Copy> {
    fn x(v: V) -> i64;
    fn y(v: V) -> i64;
    fn z(v: V) -> i64;

    fn to_real(comp: i64) -> f64;
    fn from_real(real: f64) -> i64;

    fn repr(v: V) -> String;

    fn fill(e: i64) -> V;
    fn gather(x: i64, y: i64, z: i64) -> V;
    fn scatter(v: V) -> [i64; 3];

    fn not(v: V) -> V;
    fn and(v: V, w: V) -> V;
    fn or(v: V, w: V) -> V;
    fn xor(v: V, w: V) -> V;
    fn sh(v: V, shamt: i32) -> V;

    fn add(v: V, w: V) -> V;
    fn sub(v: V, w: V) -> V;
    fn mul(v: V, w: V) -> V;
    fn div(v: V, w: V) -> V;
    fn rem(v: V, w: V) -> V;

    fn eq(v: V, w: V) -> V;
    fn equals(v: V, w: V) -> bool;
    fn lt(v: V, w: V) -> V;
    fn gt(v: V, w: V) -> V;
    fn max_comp(v: V) -> i64;
    fn min_comp(v: V) -> i64;
    fn max(v: V, w: V) -> V;
    fn min(v: V, w: V) -> V;
    fn clamp(v: V, lo: i64, hi: i64) -> V;

    fn sum(v: V) -> i64;
    fn dot(v: V, w: V) -> i64;
    fn norm1(v: V) -> i64;
    fn norm_sq(v: V) -> i64;
    fn dist1(v: V, w: V) -> i64;
    fn dist_sq(v: V, w: V) -> i64;
}
