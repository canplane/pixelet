// pxl_shared/record/table.test.rs
// (no pair: table.ts's tests are record.test.ts, mirrored beside this)

use super::*;
use xpute_core::kit::array::{F32Array, I64Array, U32Array};
use xpute_core::kit::encoding::bytes_to_hex;
use xpute_core::vector::Vector;

const VECTOR: &str = include_str!("../../vector/record/table.tsv");

#[test]
fn a_table_is_what_the_typescript_writes() {
    let v = Vector::load(VECTOR);
    let col = |key: &str, value: ColumnValue, required: bool, stride: Option<u32>| Column {
        key: key.to_string(),
        value,
        required,
        stride,
    };
    let pkt = encode_table(
        "thing",
        7,
        3,
        &[
            col("a", ColumnValue::Array(U32Array::from(&[1, 2, 0xffffffff]).view()), true, None),
            col("b", ColumnValue::Array(I64Array::from(&[-1, 0, 1 << 62]).view()), false, None),
            col("v", ColumnValue::Array(F32Array::from(&[0.5, 1.0, 1.5, 2.0, 2.5, 3.0]).view()), true, Some(2)),
            col("s", ColumnValue::Strings(vec!["".into(), "가".into(), "x".into()]), false, None),
        ],
    )
    .unwrap();
    assert_eq!(bytes_to_hex(&pkt), v.s("packet"));
    let t = Table::new(RecordSource::Packet(pkt), &["a", "b", "v", "s"]).unwrap();
    assert_eq!((t.kind.as_str(), t.version, t.count), (v.s("kind"), v.u32("version"), v.u32("count")));
    let keys: Vec<&str> = (0..v.len("keys")).map(|j| v.s(&format!("keys.{j}"))).collect();
    assert_eq!(t.keys, keys);
}
