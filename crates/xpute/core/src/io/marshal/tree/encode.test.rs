// xpute-core/io/marshal/tree/encode.test.rs
// (no pair: the TypeScript tree has no test file; the vector holds it to the
// TypeScript encoder byte for byte)

use super::*;
use crate::io::marshal::tree::cursor::{Cursor, TreeReader};
use crate::io::marshal::tree::spec::NodeValue;
use crate::io::marshal::tree::view::{NodeView, TreeView, ViewValue};
use crate::kit::array::TypedArray;
use crate::kit::encoding::bytes_to_hex;

/// What the TypeScript computed, recorded: the generator that wrote it
/// is gone and the vector stands as it is (xpute-core/sys/vector.rs).
const VECTOR: &str = include_str!("../../../../vector/io/marshal/tree/encode.tsv");

fn expected(name: &str) -> String {
    let v: std::collections::HashMap<&str, &str> = VECTOR.lines().filter_map(|l| l.split_once('\t')).collect();
    let k = (0..).find(|k| v.get(format!("{k}.name").as_str()).is_none_or(|n| *n == name)).unwrap();
    v.get(format!("{k}.packet").as_str()).unwrap_or_else(|| panic!("no vector {name}")).to_string()
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
fn a_string_written_from_display_is_the_encoder_s_string_and_reads_back_where_it_lies() {
    let mut buf = [0xffu8; 128];
    let mut w = PacketWriter::new(&mut buf, 2);
    w.str_display(format_args!("{}-{}", "héllo", 42)).u32(Some(7));
    let n = w.finish().unwrap() as usize;
    let expected = encode(|t| {
        t.branch(Some(&mut |b| {
            b.str(Some("héllo-42")).u32(Some(7));
        }));
    });
    assert_eq!(&buf[..n], &expected[..]);
    let branch = TreeReader::new(TypedArray::from(&buf[..n])).unwrap().read_branch().unwrap();
    assert_eq!(branch.at(0).unwrap().get_str_ref().unwrap(), "héllo-42");
    assert!(branch.at(1).unwrap().get_str_ref().is_err(), "a number is not a string");
    let mut small = [0u8; 40];
    let mut w = PacketWriter::new(&mut small, 1);
    w.str_display("far longer than the room that is left");
    assert_eq!(w.finish(), None, "a string that does not fit spoils the packet");
}

#[test]
fn a_packet_writer_writes_the_bytes_the_encoder_writes_for_the_same_branch() {
    let words = [7u32, 0xdead_beef, 3];
    let floats = [1.5f64, -2.25];
    let bytes = [9u8, 8, 7];
    let expected = encode(|t| {
        t.branch(Some(&mut |b| {
            b.bool(Some(true))
                .u32(Some(42))
                .str(Some("héllo, ring"))
                .u8(Some(3))
                .f64(Some(-0.5))
                .str(None)
                .u64(Some(1 << 40))
                .i32(Some(-9))
                .u32(None)
                .f32(Some(2.5))
                .u32_array(Some(&TypedArray::from(&words[..])))
                .f64_array(Some(&TypedArray::from(&floats[..])))
                .u8_array(Some(&TypedArray::from(&bytes[..])))
                .str(Some(""));
        }));
    });
    let mut buf = [0xffu8; 512];
    let mut w = PacketWriter::new(&mut buf, 14);
    w.bool(Some(true))
        .u32(Some(42))
        .str(Some("héllo, ring"))
        .u8(Some(3))
        .f64(Some(-0.5))
        .str(None)
        .u64(Some(1 << 40))
        .i32(Some(-9))
        .u32(None)
        .f32(Some(2.5))
        .u32_array(Some(&words))
        .f64_array(Some(&floats))
        .u8_array(Some(&bytes))
        .str(Some(""));
    let n = w.finish().unwrap() as usize;
    assert_eq!(&buf[..n], &expected[..]);
    let empty = encode(|t| {
        t.branch(Some(&mut |_| {}));
    });
    let n = PacketWriter::new(&mut buf, 0).finish().unwrap() as usize;
    assert_eq!(&buf[..n], &empty[..], "a branch of nothing");
    let mut small = [0u8; 40];
    let mut w = PacketWriter::new(&mut small, 1);
    w.str(Some("longer than what is left"));
    assert_eq!(w.finish(), None, "a child that does not fit spoils the packet");
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
