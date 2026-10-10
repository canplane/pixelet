// xpute_core/kit/math.test.rs
// (no pair: math.ts has no test file)

use super::*;
use crate::vector::Vector;

const VECTOR: &str = include_str!("../../vector/kit/math.tsv");

#[test]
fn math_computes_what_the_typescript_computes() {
    let v = Vector::load(VECTOR);
    v.each("", |k| {
        let (x, s) = (v.f64(&format!("{k}.x")), v.f64(&format!("{k}.s")));
        let bits = |name: &str, got: f64| assert_eq!(got.to_bits(), v.f64(&format!("{k}.{name}")).to_bits(), "{k}.{name}({x}, {s}) = {got}");
        assert_eq!(approx_eq(x, x + s * 1e-6, None), v.bool(&format!("{k}.approx_eq")), "{k}.approx_eq");
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
