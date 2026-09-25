// xpute-core/wire/xtp/encode.test.rs
// (no pair: the TypeScript tree has no test file; the vector holds it to the
// TypeScript encoder byte for byte)

use super::*;
use crate::wire::xtp::cursor::{Cursor, TreeReader};
use crate::wire::xtp::spec::{Array, NodeValue, SequenceType};
use crate::wire::xtp::view::{NodeView, TreeView, ViewValue};

/// What the TypeScript computed, recorded: the generator that wrote it
/// is gone and the vector stands as it is (xpute-core/golden.rs).
const GOLDEN: &str = include_str!("../../../golden/wire/xtp/encode.tsv");

/// A typed array as a value, written at the width its type names.
fn u16_arr<'a>(v: &[u16]) -> Array<'a> {
    Array {
        type_: SequenceType::U16_ARRAY,
        bytes: v.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>().into(),
    }
}

fn i32_arr<'a>(v: &[i32]) -> Array<'a> {
    Array {
        type_: SequenceType::I32_ARRAY,
        bytes: v.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>().into(),
    }
}

fn expected(name: &str) -> String {
    let v: std::collections::HashMap<&str, &str> = GOLDEN.lines().filter_map(|l| l.split_once('\t')).collect();
    let k = (0..).find(|k| v.get(format!("{k}.name").as_str()).is_none_or(|n| *n == name)).unwrap();
    v.get(format!("{k}.packet").as_str()).unwrap_or_else(|| panic!("no vector {name}")).to_string()
}

/// The lifetime is the caller's: a view borrows the arrays and strings it is
/// given, so leaving it to be inferred per call would make the closure answer
/// for any lifetime at all, including one outliving what it borrows.
fn encode<'a>(build: impl FnOnce(&mut TreeView<'a>)) -> Vec<u8> {
    let mut t = TreeView::new();
    build(&mut t);
    TreeEncoder::new().encode(&t, TreeEncoderOptions::default()).unwrap()
}

fn check<'a>(name: &str, build: impl FnOnce(&mut TreeView<'a>)) -> Vec<u8> {
    let pkt = encode(build);
    assert_eq!(hex::encode(&pkt), expected(name), "{name}");
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
    let branch = TreeReader::new(&buf[..n]).unwrap().read_branch().unwrap();
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
                .u32_array(Some(&words[..]))
                .f64_array(Some(&floats[..]))
                .u8_array(Some(&bytes[..]))
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
fn a_branch_inside_a_packet_writer_is_the_encoder_s_branch() {
    let floats = [0.5f64, 4.0];
    let expected = encode(|t| {
        t.branch(Some(&mut |b| {
            b.u32(Some(1))
                .branch(Some(&mut |b| {
                    b.str(Some("a")).branch(Some(&mut |b| {
                        b.f64(Some(2.0)).str(None);
                    }));
                }))
                .branch(None)
                .branch(Some(&mut |_| {}))
                .f64_array(Some(&floats[..]));
        }));
    });
    let mut buf = [0xffu8; 512];
    let mut w = PacketWriter::new(&mut buf, 5);
    w.u32(Some(1))
        .branch(2, |w| {
            w.str(Some("a")).branch(2, |w| {
                w.f64(Some(2.0)).str(None);
            });
        })
        .no_branch()
        .branch(0, |_| {})
        .f64_array(Some(&floats));
    let n = w.finish().unwrap() as usize;
    assert_eq!(&buf[..n], &expected[..]);
    let root = TreeReader::new(&buf[..n]).unwrap().read_branch().unwrap();
    let inner = root.at_branch(1).unwrap().at_branch(1).unwrap();
    assert_eq!(inner.at(0).unwrap().get_number().unwrap(), 2.0);
    let mut small = [0u8; 48];
    let mut w = PacketWriter::new(&mut small, 1);
    w.branch(3, |w| {
        w.u32(Some(1)).u32(Some(2)).str(Some("past the room"));
    });
    assert_eq!(w.finish(), None, "a branch that does not fit spoils the packet");
}

#[test]
fn the_encoder_writes_the_bytes_the_typescript_writes() {
    check("nil", |t| {
        t.nil();
    });
    check("i32", |t| {
        t.set(ViewValue::I32(42)).unwrap();
    });
    check("f64", |t| {
        t.set(ViewValue::F64(1.5)).unwrap();
    });
    check("bigint", |t| {
        t.set(ViewValue::I64(-5)).unwrap();
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
        t.set(ViewValue::Array(u16_arr(&[1, 2, 3]))).unwrap();
    });
    check("typed f32", |t| {
        t.f32_array(Some(&[0.5, -2.0]));
    });
    check("bitset", |t| {
        t.bitset(Some(&[1, 0, 1, 1, 0, 0, 0, 0, 1]));
    });
    check("branch mixed", |t| {
        t.set(ViewValue::List(vec![
            ViewValue::I32(42),
            ViewValue::Str("hello".into()),
            ViewValue::Bool(true),
            ViewValue::Null,
            ViewValue::I64(1 << 40),
            ViewValue::List(vec![ViewValue::I32(1), ViewValue::I32(2)]),
            ViewValue::Array(i32_arr(&[7])),
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
        t.set(ViewValue::List(vec![ViewValue::I32(1), ViewValue::Str("x".into())])).unwrap();
    });
    check("graft", |t| {
        t.graft(&inner).unwrap();
    });
}

#[test]
fn a_packet_reads_back_what_was_written() {
    let pkt = encode(|t| {
        t.set(ViewValue::List(vec![
            ViewValue::I32(42),
            ViewValue::Str("hello".into()),
            ViewValue::Bool(true),
            ViewValue::Null,
            ViewValue::List(vec![ViewValue::I32(1), ViewValue::I64(-2)]),
            ViewValue::Array(i32_arr(&[7, 8])),
        ]))
        .unwrap();
    });
    let reader = TreeReader::new(&pkt).unwrap();
    let root = reader.read_branch().unwrap();
    assert_eq!(root.len, 6);
    let deep = root.get_deep().unwrap();
    assert_eq!(
        deep,
        NodeValue::List(vec![
            NodeValue::I32(42),
            NodeValue::Str("hello".into()),
            NodeValue::Bool(true),
            NodeValue::Null,
            NodeValue::List(vec![NodeValue::I32(1), NodeValue::I64(-2)]),
            NodeValue::Array(i32_arr(&[7, 8])),
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
    let root = TreeReader::new(&pkt).unwrap().read_branch().unwrap();
    let Cursor::Null(n) = root.at(0).unwrap() else { panic!("a null cursor") };
    assert_eq!(n.0.type_, ScalarType::U32 as u8);
    assert!(root.at(0).unwrap().opt().is_none());
}
