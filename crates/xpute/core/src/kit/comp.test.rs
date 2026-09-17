// xpute_core/kit/comp.test.rs
// (no pair: comp.ts has no test file)

use super::*;
use crate::sys::vector::Vector;

const VECTOR: &str = include_str!("../../vector/kit/comp.tsv");

/// A string field, which the flattening leaves out when it is empty.
fn text(v: &Vector, key: &str) -> &'static str {
    if v.has(key) {
        v.s(key)
    } else {
        ""
    }
}

#[test]
fn comp_orders_what_the_typescript_orders() {
    let v = Vector::load(VECTOR);
    v.each("str", |k| {
        let (a, b) = (text(&v, &format!("{k}.a")), text(&v, &format!("{k}.b")));
        assert_eq!(comp_str(a, b), v.i32(&format!("{k}.c")), "comp_str({a:?}, {b:?})");
    });
    let numeric = [
        comp_numeric(1.0, 2.0),
        comp_numeric(2.0, 1.0),
        comp_numeric(0.0, -0.0),
        comp_numeric(f64::NAN, 1.0),
        comp_numeric(1i128, 2i128),
    ];
    for (j, c) in numeric.into_iter().enumerate() {
        assert_eq!(c, v.i32(&format!("numeric.{j}")), "numeric.{j}");
    }
}
