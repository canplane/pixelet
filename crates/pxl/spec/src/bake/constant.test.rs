// pxl-spec/bake/constant.test.rs
// (no pair: constant.ts has no test file)

use xpute_core::vector::Vector;

use super::*;

const VECTOR: &str = include_str!("../../vector/bake/constant.tsv");

#[test]
fn constants_are_what_the_typescript_computes() {
    let v = Vector::load(VECTOR);
    assert_eq!(CELL_SHAMT, v.i32("CELL_SHAMT"));
    assert_eq!(U_PER_CELL.to_bits(), v.f64("U_PER_CELL").to_bits());
    assert_eq!(XY_CELL_PER_M.to_bits(), v.f64("XY_CELL_PER_M").to_bits());
    for (j, y) in [0.0, 1234567.8, -4500000.0, 12000000.0].into_iter().enumerate() {
        assert_eq!(Z_CELL_PER_M(y).to_bits(), v.f64(&format!("Z_CELL_PER_M.{j}")).to_bits(), "Z_CELL_PER_M({y})");
    }
    assert_eq!(PAGE_CELL_SPAN, v.u32("PAGE_CELL_SPAN"));
    assert_eq!(CHUNK_CELL_SPAN, v.u32("CHUNK_CELL_SPAN"));
    assert_eq!(PAGE_ELE_SPAN, v.u32("PAGE_ELE_SPAN"));
    assert_eq!(ELE_PAD, v.u32("ELE_PAD"));
    assert_eq!(ELE_PADDED_SPAN, v.u32("ELE_PADDED_SPAN"));
    assert_eq!(
        (OBJECT_LAYOUT.dim, OBJECT_LAYOUT.min, OBJECT_LAYOUT.max),
        (v.u32("OBJECT_LAYOUT.dim"), v.i32("OBJECT_LAYOUT.min"), v.i32("OBJECT_LAYOUT.max"))
    );
}
