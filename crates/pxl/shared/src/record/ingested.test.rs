// pxl_shared/record/ingested.test.rs
// (no pair: ingested.ts is generated and has no test file)

use super::*;
use xpute_core::vector::Vector;

const VECTOR: &str = include_str!("../../vector/record/ingested.tsv");

#[test]
fn the_generated_table_is_the_typescript_s_row_for_row() {
    let v = Vector::load(VECTOR);
    let mut n = 0;
    v.each("rows", |k| {
        n += 1;
        let path = v.s(&format!("{k}.path"));
        let a = INGESTED.get(path).unwrap_or_else(|| panic!("{k}: {path} is not in ingested.rs"));
        assert_eq!((a.key, a.bytes), (v.s(&format!("{k}.key")), v.u64(&format!("{k}.bytes"))), "{k}");
        match a.extent {
            None => assert_eq!("none", v.s(&format!("{k}.extent")), "{k}.extent"),
            Some(e) => {
                for (i, x) in e.iter().enumerate() {
                    assert_eq!(x.to_bits(), v.f64(&format!("{k}.extent.{i}")).to_bits(), "{k}.extent.{i}");
                }
            }
        }
    });
    assert_eq!(n, INGESTED.len());
    assert_eq!(
        (SHEET.key, SHEET.bytes, SHEET.cell, SHEET.columns),
        (v.s("sheet.key"), v.u64("sheet.bytes"), v.u32("sheet.cell"), v.u32("sheet.columns"))
    );
    let tiles: Vec<&str> = (0..v.len("sheet.tiles")).map(|j| v.s(&format!("sheet.tiles.{j}"))).collect();
    assert_eq!(SHEET.tiles, &tiles[..]);
}
