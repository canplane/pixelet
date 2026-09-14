// xpute-core/io/marshal/tree/encode.test.rs
// (no pair: the TypeScript tree has no test file; the vector holds it to the
// TypeScript encoder byte for byte)

use super::*;
use crate::io::marshal::tree::cursor::{Cursor, TreeReader};
use crate::io::marshal::tree::spec::NodeValue;
use crate::io::marshal::tree::view::{NodeView, TreeView, ViewValue};
use crate::kit::array::TypedArray;
use crate::kit::encoding::bytes_to_hex;

/// script/vector/xpute/core/io/marshal/tree/encode.ts's output.
const VECTOR: &str = include_str!("../../../../vector/io/marshal/tree/encode.json");

fn expected(name: &str) -> String {
    // The vector is a JSON list of {name, packet}; read without a JSON
    // dependency, since the shape is fixed.
    let key = format!("\"name\": \"{name}\"");
    let at = VECTOR.find(&key).unwrap_or_else(|| panic!("no vector {name}"));
    let rest = &VECTOR[at..];
    let p = rest.find("\"packet\": \"").unwrap() + "\"packet\": \"".len();
    let end = rest[p..].find('"').unwrap();
    rest[p..p + end].to_string()
}

fn encode(build: impl FnOnce(&mut TreeView)) -> Vec<u8> {
    let mut t = TreeView::new();
    build(&mut t);
    TreeEncoder::new().encode(&t, TreeEncoderOptions::default()).unwrap().as_slice().to_vec()
}

fn check(name: &str, build: impl FnOnce(&mut TreeView)) -> Vec<u8> {
    let pkt = encode(build);
    assert_eq!(bytes_to_hex(&TypedArray::from(&pkt)), expected(name), "{name}");
    pkt
}

#[test]
fn the_encoder_writes_the_bytes_the_typescript_writes() {
    check("nil", |t| {
        t.nil();
    });
    check("i32", |t| {
        t.set(ViewValue::Number(42.0)).unwrap();
    });
    check("f64", |t| {
        t.set(ViewValue::Number(1.5)).unwrap();
    });
    check("bigint", |t| {
        t.set(ViewValue::BigInt(-5)).unwrap();
    });
    check("u64", |t| {
        t.u64(Some(0xffff_ffff_ffff_fffe));
    });
    check("bool", |t| {
        t.set(ViewValue::Bool(true)).unwrap();
    });
    check("str", |t| {
        t.set(ViewValue::Str("héllo".into())).unwrap();
    });
    check("str empty", |t| {
        t.str(Some(""));
    });
    check("typed u16", |t| {
        t.set(ViewValue::Array(TypedArray::<u16>::from(&[1, 2, 3]).view())).unwrap();
    });
    check("typed f32", |t| {
        t.f32_array(Some(&TypedArray::from(&[0.5, -2.0])));
    });
    check("bitset", |t| {
        t.bitset(Some(&TypedArray::from(&[1, 0, 1, 1, 0, 0, 0, 0, 1])));
    });
    check("branch mixed", |t| {
        t.set(ViewValue::List(vec![
            ViewValue::Number(42.0),
            ViewValue::Str("hello".into()),
            ViewValue::Bool(true),
            ViewValue::Null,
            ViewValue::BigInt(1 << 40),
            ViewValue::List(vec![ViewValue::Number(1.0), ViewValue::Number(2.0)]),
            ViewValue::Array(TypedArray::<i32>::from(&[7]).view()),
        ]))
        .unwrap();
    });
    check("branch null child", |t| {
        t.branch(Some(&mut |b| {
            b.u8(None).str(None).branch(None).u32(Some(9));
        }));
    });
    check("branch empty", |t| {
        t.set(ViewValue::List(vec![])).unwrap();
    });
    check("typed null root", |t| {
        t.u32(None);
    });
    let inner = encode(|t| {
        t.set(ViewValue::List(vec![ViewValue::Number(1.0), ViewValue::Str("x".into())])).unwrap();
    });
    check("graft", |t| {
        t.graft(TypedArray::from(&inner)).unwrap();
    });
}

#[test]
fn a_packet_reads_back_what_was_written() {
    let pkt = encode(|t| {
        t.set(ViewValue::List(vec![
            ViewValue::Number(42.0),
            ViewValue::Str("hello".into()),
            ViewValue::Bool(true),
            ViewValue::Null,
            ViewValue::List(vec![ViewValue::Number(1.0), ViewValue::BigInt(-2)]),
            ViewValue::Array(TypedArray::<i32>::from(&[7, 8]).view()),
        ]))
        .unwrap();
    });
    let reader = TreeReader::new(TypedArray::from(&pkt)).unwrap();
    let root = reader.read_branch().unwrap();
    assert_eq!(root.len, 6);
    let deep = root.get_deep().unwrap();
    assert_eq!(
        deep,
        NodeValue::List(vec![
            NodeValue::Number(42.0),
            NodeValue::Str("hello".into()),
            NodeValue::Bool(true),
            NodeValue::Null,
            NodeValue::List(vec![NodeValue::Number(1.0), NodeValue::BigInt(-2)]),
            NodeValue::Array(TypedArray::<i32>::from(&[7, 8]).view()),
        ])
    );
    // A shallow read keeps the inner branch as a cursor.
    let NodeValue::List(shallow) = root.get().unwrap() else { panic!() };
    assert!(matches!(shallow[4], NodeValue::Extra(_)));
    // A null child preserves its type and reads as null.
    let pkt = encode(|t| {
        t.branch(Some(&mut |b| {
            b.u32(None);
        }));
    });
    let root = TreeReader::new(TypedArray::from(&pkt)).unwrap().read_branch().unwrap();
    let Cursor::Null(n) = root.at(0).unwrap() else { panic!("a null cursor") };
    assert_eq!(n.0.type_, ScalarType::U32 as u8);
    assert!(root.at(0).unwrap().opt().is_none());
}
