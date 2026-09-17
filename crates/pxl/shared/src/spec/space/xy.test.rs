// pxl_shared/spec/space/xy.test.rs
// (no pair: xy.ts has no test file)

use xpute_core::sys::vector::Vector;

use super::*;

const VECTOR: &str = include_str!("../../../vector/spec/space/xy.tsv");

#[test]
fn xy_computes_what_the_typescript_computes() {
    let v = Vector::load(VECTOR);
    v.each("words", |k| {
        let i = v.u64(&format!("{k}.i"));
        assert_eq!(TO_XY(i), v.u64(&format!("{k}.TO_XY")), "{k}.TO_XY");
        assert_eq!(WITH_X(-7, i), v.u64(&format!("{k}.WITH_X")), "{k}.WITH_X");
        assert_eq!(WITH_Y(9, i), v.u64(&format!("{k}.WITH_Y")), "{k}.WITH_Y");
        assert_eq!(WITH_Z(-32768, i), v.u64(&format!("{k}.WITH_Z")), "{k}.WITH_Z");
    });
    assert_eq!(CELL_CENTER_U(1.0).to_bits(), v.f64("CELL_CENTER_U").to_bits());
}
