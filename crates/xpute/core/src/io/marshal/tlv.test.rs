// xpute_core/io/marshal/tlv.test.rs
// (no pair: tlv.ts has no test file)

use super::*;
use crate::kit::encoding::bytes_to_hex;
use crate::vector::Vector;

const VECTOR: &str = include_str!("../../../vector/io/marshal/tlv.tsv");

/// A read value as the TypeScript prints it: `typeof`, a colon, the value.
fn show(v: &TLVValue) -> String {
    match v {
        TLVValue::Bool(b) => format!("boolean:{b}"),
        TLVValue::U8(x) => format!("number:{x}"),
        TLVValue::I8(x) => format!("number:{x}"),
        TLVValue::U16(x) => format!("number:{x}"),
        TLVValue::I16(x) => format!("number:{x}"),
        TLVValue::U32(x) => format!("number:{x}"),
        TLVValue::I32(x) => format!("number:{x}"),
        TLVValue::U64(x) => format!("bigint:{x}"),
        TLVValue::I64(x) => format!("bigint:{x}"),
        TLVValue::F32(x) if x.is_nan() => "number:NaN".to_string(),
        TLVValue::F64(x) if x.is_nan() => "number:NaN".to_string(),
        TLVValue::F32(x) => format!("number:{}", *x as f64),
        TLVValue::F64(x) => format!("number:{x}"),
        TLVValue::Number(x) => format!("number:{x}"),
        TLVValue::BigInt(x) => format!("bigint:{x}"),
        TLVValue::Str(s) => format!("string:{s}"),
        TLVValue::Bytes(b) => format!("bytes:{}", bytes_to_hex(b)),
    }
}

#[test]
fn tlv_computes_what_the_typescript_computes() {
    let v = Vector::load(VECTOR);
    let mut w = TLVWriter::new(4);
    w.u8(200).unwrap().i8(-3).unwrap().u16(65535).unwrap().i16(-2).unwrap().u32(4000000000).unwrap().i32(-7).unwrap();
    w.u64(1 << 63).unwrap().i64(-(1 << 40)).unwrap().f32(0.25).unwrap().f64(-1.5).unwrap().bool(true).unwrap();
    w.str("한글").unwrap().bytes(&TypedArray::from(&[9, 8, 7])).unwrap();
    w.write_all(&[
        TLVValue::Bool(true),
        TLVValue::Number(5.0),
        TLVValue::Number(-2147483648.0),
        TLVValue::Number(2147483648.0),
        TLVValue::Number(9007199254740991.0),
        TLVValue::Number(1.5),
        TLVValue::Number(f64::NAN),
        TLVValue::BigInt(-(1i128 << 63)),
        TLVValue::Str("s".into()),
        TLVValue::Bytes(TypedArray::from(&[1])),
    ])
    .unwrap();
    let packet = w.finish();
    assert_eq!(bytes_to_hex(&packet), v.s("packet"));
    let read: Vec<String> = TLVReader::new(packet).read_all().unwrap().iter().map(show).collect();
    let want: Vec<&str> = (0..v.len("read")).map(|j| v.s(&format!("read.{j}"))).collect();
    assert_eq!(read, want);

    let huge = TLVReader::new(crate::kit::encoding::hex_to_bytes(v.s("huge.packet"))).read_all();
    assert_eq!(huge.map(|_| "ok".to_string()).unwrap_or_else(|e| format!("errno:{}", e.errno as i32)), v.s("huge.read"));
}
