// xpute-core/wire/tlv.test.rs
// (no pair: tlv.ts has no test file)

use super::*;
use crate::golden::Golden;

const GOLDEN: &str = include_str!("../../golden/wire/tlv.tsv");

/// A read value as the TypeScript prints it: `typeof`, a colon, the value.
fn show(v: &TlvValue) -> String {
    match v {
        TlvValue::Bool(b) => format!("boolean:{b}"),
        TlvValue::U8(x) => format!("number:{x}"),
        TlvValue::I8(x) => format!("number:{x}"),
        TlvValue::U16(x) => format!("number:{x}"),
        TlvValue::I16(x) => format!("number:{x}"),
        TlvValue::U32(x) => format!("number:{x}"),
        TlvValue::I32(x) => format!("number:{x}"),
        TlvValue::U64(x) => format!("bigint:{x}"),
        TlvValue::I64(x) => format!("bigint:{x}"),
        TlvValue::F32(x) if x.is_nan() => "number:NaN".to_string(),
        TlvValue::F64(x) if x.is_nan() => "number:NaN".to_string(),
        TlvValue::F32(x) => format!("number:{}", *x as f64),
        TlvValue::F64(x) => format!("number:{x}"),
        TlvValue::Str(s) => format!("string:{s}"),
        TlvValue::Bytes(b) => format!("bytes:{}", hex::encode(b)),
    }
}

#[test]
fn tlv_computes_what_the_typescript_computes() {
    let v = Golden::load(GOLDEN);
    let mut w = TlvWriter::new(4);
    w.u8(200).unwrap().i8(-3).unwrap().u16(65535).unwrap().i16(-2).unwrap().u32(4000000000).unwrap().i32(-7).unwrap();
    w.u64(1 << 63).unwrap().i64(-(1 << 40)).unwrap().f32(0.25).unwrap().f64(-1.5).unwrap().bool(true).unwrap();
    w.str("한글").unwrap().bytes(&[9, 8, 7]).unwrap();
    w.write_all(&[
        TlvValue::Bool(true),
        TlvValue::I32(5),
        TlvValue::I32(-2147483648),
        TlvValue::I64(2147483648),
        TlvValue::I64(9007199254740991),
        TlvValue::F64(1.5),
        TlvValue::F64(f64::NAN),
        TlvValue::I64(i64::MIN),
        TlvValue::Str("s".into()),
        TlvValue::Bytes(vec![1]),
    ])
    .unwrap();
    let packet = w.finish();
    assert_eq!(hex::encode(packet), v.s("packet"));
    let read: Vec<String> = TlvReader::new(packet).read_all().unwrap().iter().map(show).collect();
    let want: Vec<&str> = (0..v.len("read")).map(|j| v.s(&format!("read.{j}"))).collect();
    assert_eq!(read, want);

    let huge_pkt = hex::decode(v.s("huge.packet")).unwrap();
    let huge = TlvReader::new(&huge_pkt).read_all();
    assert_eq!(huge.map(|_| "ok".to_string()).unwrap_or_else(|e| format!("errno:{}", e.errno as i32)), v.s("huge.read"));
}
