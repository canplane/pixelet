// pxl-spec/space/pvf3/pvec.rs

//! The packed word the two vector families share, and the one operation
//! contract both modules meet: `PVecTraits`, implemented by a marker type per
//! module so a caller can be generic over which word it packs.

use super::fvec::{self, FVec};
use super::ivec::{self, IVec};

/// Packed Word Vector Union
/// Collective union of the packed-word vector families: ivec and fvec.
pub type PVec = u64;

/// Common operation contract implemented by the packed-word vector modules
/// (ivec and fvec).
#[allow(non_snake_case)]
pub trait PVecTraits {
    type V;

    // ---- Components ---

    fn X(v: Self::V) -> i64;
    fn Y(v: Self::V) -> i64;
    fn Z(v: Self::V) -> i64;

    fn to_real(comp: i64) -> f64;
    fn from_real(real: f64) -> i64;

    // ---- Logging ----

    fn repr(v: Self::V) -> String;

    // ---- Construction ----

    fn fill(e: i64) -> Self::V;
    fn gather(x: i64, y: i64, z: i64) -> Self::V;
    fn scatter(out: &mut [i64], v: Self::V);

    // ---- Bitwise ----

    fn not(v: Self::V) -> Self::V; // bitwise NOT within the module's packed word width
    fn and(v: Self::V, w: Self::V) -> Self::V;
    fn or(v: Self::V, w: Self::V) -> Self::V;
    fn xor(v: Self::V, w: Self::V) -> Self::V;

    /// Signed shift: +k shifts left by k, -k right by k, per lane.
    /// - ivec: lanes are signed, so a right shift is arithmetic.
    /// - fvec: lanes are non-negative residues, so the arithmetic shift is a
    ///   logical one.
    ///
    /// No guards: a shift past the word is the caller's bug.
    fn sh(v: Self::V, shamt: i32) -> Self::V;

    // ---- Arithmetic ----

    fn add(v: Self::V, w: Self::V) -> Self::V;
    fn sub(v: Self::V, w: Self::V) -> Self::V;
    fn mul(v: Self::V, w: Self::V) -> Self::V;
    fn div(v: Self::V, w: Self::V) -> Self::V;
    fn r#mod(v: Self::V, w: Self::V) -> Self::V;

    // ---- Comparison ----

    fn eq(v: Self::V, w: Self::V) -> Self::V; // packed per-lane boolean mask
    fn equals(v: Self::V, w: Self::V) -> bool;
    fn lt(v: Self::V, w: Self::V) -> Self::V;
    fn gt(v: Self::V, w: Self::V) -> Self::V;
    fn max_comp(v: Self::V) -> i64;
    fn min_comp(v: Self::V) -> i64;
    fn max(v: Self::V, w: Self::V) -> Self::V;
    fn min(v: Self::V, w: Self::V) -> Self::V;
    fn clamp(v: Self::V, min: i64, max: i64) -> Self::V;

    // ---- Reduction ----

    fn sum(v: Self::V) -> i64;
    fn dot(v: Self::V, w: Self::V) -> i64;
    fn norm1(v: Self::V) -> i64;
    fn norm_sq(v: Self::V) -> i64;
    fn dist1(v: Self::V, w: Self::V) -> i64;
    fn dist_sq(v: Self::V, w: Self::V) -> i64;
}

/// `ivec satisfies PVecTraits<IVec>`: the module as a type.
pub struct Ivec;
/// `fvec satisfies PVecTraits<FVec>`.
pub struct Fvec;

macro_rules! satisfies {
    ($marker:ident, $module:ident, $v:ty) => {
        impl PVecTraits for $marker {
            type V = $v;
            fn X(v: $v) -> i64 {
                $module::X(v)
            }
            fn Y(v: $v) -> i64 {
                $module::Y(v)
            }
            fn Z(v: $v) -> i64 {
                $module::Z(v)
            }
            fn to_real(comp: i64) -> f64 {
                $module::to_real(comp)
            }
            fn from_real(real: f64) -> i64 {
                $module::from_real(real)
            }
            fn repr(v: $v) -> String {
                $module::repr(v)
            }
            fn fill(e: i64) -> $v {
                $module::fill(e)
            }
            fn gather(x: i64, y: i64, z: i64) -> $v {
                $module::gather(x, y, z)
            }
            fn scatter(out: &mut [i64], v: $v) {
                $module::scatter(out, v)
            }
            fn not(v: $v) -> $v {
                $module::not(v)
            }
            fn and(v: $v, w: $v) -> $v {
                $module::and(v, w)
            }
            fn or(v: $v, w: $v) -> $v {
                $module::or(v, w)
            }
            fn xor(v: $v, w: $v) -> $v {
                $module::xor(v, w)
            }
            fn sh(v: $v, shamt: i32) -> $v {
                $module::sh(v, shamt)
            }
            fn add(v: $v, w: $v) -> $v {
                $module::add(v, w)
            }
            fn sub(v: $v, w: $v) -> $v {
                $module::sub(v, w)
            }
            fn mul(v: $v, w: $v) -> $v {
                $module::mul(v, w)
            }
            fn div(v: $v, w: $v) -> $v {
                $module::div(v, w)
            }
            fn r#mod(v: $v, w: $v) -> $v {
                $module::r#mod(v, w)
            }
            fn eq(v: $v, w: $v) -> $v {
                $module::eq(v, w)
            }
            fn equals(v: $v, w: $v) -> bool {
                $module::equals(v, w)
            }
            fn lt(v: $v, w: $v) -> $v {
                $module::lt(v, w)
            }
            fn gt(v: $v, w: $v) -> $v {
                $module::gt(v, w)
            }
            fn max_comp(v: $v) -> i64 {
                $module::max_comp(v)
            }
            fn min_comp(v: $v) -> i64 {
                $module::min_comp(v)
            }
            fn max(v: $v, w: $v) -> $v {
                $module::max(v, w)
            }
            fn min(v: $v, w: $v) -> $v {
                $module::min(v, w)
            }
            fn clamp(v: $v, min: i64, max: i64) -> $v {
                $module::clamp(v, min, max)
            }
            fn sum(v: $v) -> i64 {
                $module::sum(v)
            }
            fn dot(v: $v, w: $v) -> i64 {
                $module::dot(v, w)
            }
            fn norm1(v: $v) -> i64 {
                $module::norm1(v)
            }
            fn norm_sq(v: $v) -> i64 {
                $module::norm_sq(v)
            }
            fn dist1(v: $v, w: $v) -> i64 {
                $module::dist1(v, w)
            }
            fn dist_sq(v: $v, w: $v) -> i64 {
                $module::dist_sq(v, w)
            }
        }
    };
}

satisfies!(Ivec, ivec, IVec);
satisfies!(Fvec, fvec, FVec);
