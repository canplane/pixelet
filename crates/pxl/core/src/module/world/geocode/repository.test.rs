// pxl_core/module/world/geocode/repository.test.rs

use super::*;
use crate::module::world::geocode::label::assemble_label;

#[test]
fn a_geocode_answer_reads_as_rows_of_parts_and_anything_else_as_none() {
    let mut t = PartTable::new(8, 16);
    let rows = parse_geocode(&mut t, r#"{"parts":[{"name":"강남구","common":{"en":"Gangnam-gu"}},{"name":"역삼1동"}]}"#).unwrap();
    assert_eq!(rows.count, 2);
    assert_eq!(assemble_label(&t, rows, "ko-KR").to_string(), "강남구 역삼1동");
    assert_eq!(assemble_label(&t, rows, "en").to_string(), "역삼1동, Gangnam-gu");
    assert_eq!(parse_geocode(&mut t, r#"{"parts":{}}"#), None);
    assert_eq!(parse_geocode(&mut t, "not json"), None);

    let mut small = PartTable::new(1, 1);
    let rows = parse_geocode(&mut small, r#"{"parts":[{"name":"강남구"},{"name":"역삼1동"}]}"#).unwrap();
    assert_eq!(rows.count, 2, "a table's sizes are where it starts, not a limit");
    assert_eq!(assemble_label(&small, rows, "ko").to_string(), "강남구 역삼1동");
}
