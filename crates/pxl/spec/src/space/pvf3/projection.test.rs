// pxl-spec/space/pvf3/projection.test.rs
// (no pair: projection.ts has no test file)

use xpute_core::sys::vector::Vector;

use super::*;

const VECTOR: &str = include_str!("../../../vector/spec/space/pvf3/projection.tsv");

#[test]
fn projection_computes_what_the_typescript_computes() {
    let v = Vector::load(VECTOR);
    v.each("", |k| {
        let (x, y, z) = (v.f64(&format!("{k}.x")), v.f64(&format!("{k}.y")), v.f64(&format!("{k}.z")));
        let mut p = PVF3::default();
        project(&mut p, x, y, z);
        assert_eq!((p.i, p.f), (v.u64(&format!("{k}.i")), v.u64(&format!("{k}.f"))), "{k}.project");
        let mut u = [0.0; 3];
        unproject(&mut u, p.i, p.f);
        for (a, got) in u.iter().enumerate() {
            assert_eq!(got.to_bits(), v.f64(&format!("{k}.u.{a}")).to_bits(), "{k}.unproject[{a}]");
        }
    });
}
