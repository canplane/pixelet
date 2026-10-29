// xpute-kit/math/scalar.test.rs

use super::*;
use crate::golden::Golden;

const GOLDEN: &str = include_str!("../../../../spec/golden/math/scalar.tsv");

#[test]
fn matches_record() {
    let v = Golden::load(GOLDEN);
    v.each("", |k| {
        let (x, s) = (v.f64(&format!("{k}.x")), v.f64(&format!("{k}.s")));
        let bits = |name: &str, got: f64| {
            assert_eq!(
                got.to_bits(),
                v.f64(&format!("{k}.{name}")).to_bits(),
                "{k}.{name}({x}, {s}) = {got}"
            )
        };
        assert_eq!(
            approx_eq(x, x + s * 1e-6, None),
            v.bool(&format!("{k}.approx_eq")),
            "{k}.approx_eq"
        );
        assert_eq!(near(x, s, 0.3), v.bool(&format!("{k}.near")), "{k}.near");
        bits("lerp", lerp(x, s, 0.3));
        bits("inv_lerp", inv_lerp(x, -s, s * 2.0));
        bits("remap", remap(x, -1.0, 1.0, 0.0, s));
        bits("clamp", clamp(x, -s, s));
        bits("wrap", wrap(x, -180.0, 180.0));
        bits("floor_to_step", floor_to_step(x, s));
        bits("round_to_step", round_to_step(x, s));
        bits("ceil_to_step", ceil_to_step(x, s));
    });
}

#[test]
fn record_tolerance() {
    assert_eq!(EPS.to_bits(), Golden::load(GOLDEN).f64("eps").to_bits());
}

#[test]
fn half_rounds_up() {
    assert_eq!(round_half_up(2.5), 3.0);
    assert_eq!(round_half_up(-2.5), -2.0);
    assert_eq!(round_half_up(-2.4), -2.0);
    assert_eq!(round_half_up(-2.6), -3.0);
    assert_eq!(round_half_up(-0.5), 0.0);
    assert_eq!(round_half_up(0.49999999999999994), 0.0);
}
