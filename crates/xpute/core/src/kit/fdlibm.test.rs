// xpute_core/kit/fdlibm.test.rs
// (no pair: fdlibm.rs is JavaScript's Math, ported)

use super::*;
use crate::sys::vector::Vector;

/// script/vector/xpute/core/kit/fdlibm.ts's output: V8's Math.
const VECTOR: &str = include_str!("../../vector/kit/fdlibm.tsv");

/// How many representable doubles lie between `a` and `b`: 0 when equal,
/// NaN with NaN, and a large number across a sign or a special.
fn ulps(a: f64, b: f64) -> u64 {
    if a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan()) {
        return 0;
    }
    if a.is_sign_negative() != b.is_sign_negative() || !a.is_finite() || !b.is_finite() {
        return u64::MAX;
    }
    a.abs().to_bits().abs_diff(b.abs().to_bits())
}

/// The kernel's doubles are its own (the module doc): the transcendental
/// functions are held within one ulp of V8's correctly rounded ones, and
/// everything JavaScript computes exactly — the cube root, the hypot, the
/// round — to the bit.
#[test]
fn math_is_v8_s_to_the_bit_and_the_transcendental_functions_to_one_ulp() {
    let v = Vector::load(VECTOR);
    let mut off_by_one = 0;
    v.each("", |k| {
        let (x, y) = (v.f64(&format!("{k}.x")), v.f64(&format!("{k}.y")));
        let near = |name: &str, got: f64, off: &mut u32| {
            let want = v.f64(&format!("{k}.{name}"));
            let d = ulps(got, want);
            assert!(d <= 1, "{name}({x:e}, {y:e}) = {got:e}, V8 {want:e}");
            *off += d as u32;
        };
        near("sin", sin(x), &mut off_by_one);
        near("cos", cos(x), &mut off_by_one);
        near("tan", tan(x), &mut off_by_one);
        near("atan", atan(x), &mut off_by_one);
        near("atan2", atan2(y, x), &mut off_by_one);
        near("exp", exp(x), &mut off_by_one);
        near("log", log(x), &mut off_by_one);
        let exact = |name: &str, got: f64| {
            let want = v.f64(&format!("{k}.{name}"));
            assert_eq!(ulps(got, want), 0, "{name}({x:e}, {y:e}) = {got:e}, V8 {want:e}");
        };
        exact("cbrt", cbrt(x));
        exact("round", round(x));
        exact("hypot", hypot(&[x, y, x * 0.5]));
    });
    assert!(off_by_one < v.len("") as u32, "one ulp is the exception, not the rule: {off_by_one}");
}
