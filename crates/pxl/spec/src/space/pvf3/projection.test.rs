// pxl-spec/space/pvf3/projection.test.rs

use xpute_core::golden::Golden;

use super::*;

const GOLDEN: &str = include_str!("../../../golden/spec/space/pvf3/projection.tsv");

#[test]
fn projection_computes_what_the_record_holds() {
    let v = Golden::load(GOLDEN);
    v.each("", |k| {
        let (x, y, z) = (v.f64(&format!("{k}.x")), v.f64(&format!("{k}.y")), v.f64(&format!("{k}.z")));
        let mut p = Pvf3::default();
        project(&mut p, x, y, z);
        assert_eq!((p.i, p.f), (v.u64(&format!("{k}.i")), v.u32(&format!("{k}.f"))), "{k}.project");
        let mut u = [0.0; 3];
        unproject(&mut u, p.i, p.f);
        for (a, got) in u.iter().enumerate() {
            assert_eq!(got.to_bits(), v.f64(&format!("{k}.u.{a}")).to_bits(), "{k}.unproject[{a}]");
        }
    });
}
