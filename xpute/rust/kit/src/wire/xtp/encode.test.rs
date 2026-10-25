// xpute-kit/wire/xtp/encode.test.rs

use super::*;
use crate::wire::xtp::cursor::TreeReader;
use crate::wire::xtp::spec::{Array, SequenceType};
use crate::wire::xtp::view::{NodeView, TreeView, ViewValue};

const GOLDEN: &str = include_str!("../../../../../spec/golden/wire/xtp/encode.tsv");

fn u16_arr<'a>(v: &[u16]) -> Array<'a> {
    Array {
        type_: SequenceType::U16_ARRAY,
        bytes: v
            .iter()
            .flat_map(|x| x.to_le_bytes())
            .collect::<Vec<u8>>()
            .into(),
    }
}

fn i32_arr<'a>(v: &[i32]) -> Array<'a> {
    Array {
        type_: SequenceType::I32_ARRAY,
        bytes: v
            .iter()
            .flat_map(|x| x.to_le_bytes())
            .collect::<Vec<u8>>()
            .into(),
    }
}

fn expected(name: &str) -> String {
    let v: std::collections::HashMap<&str, &str> =
        GOLDEN.lines().filter_map(|l| l.split_once('\t')).collect();
    let k = (0..)
        .find(|k| {
            v.get(format!("{k}.name").as_str())
                .is_none_or(|n| *n == name)
        })
        .unwrap();
    v.get(format!("{k}.packet").as_str())
        .unwrap_or_else(|| panic!("no vector {name}"))
        .to_string()
}

/// `'a` is named so the closure is not asked to work for every lifetime.
fn encode<'a>(build: impl FnOnce(&mut TreeView<'a>)) -> Vec<u8> {
    let mut t = TreeView::new();
    build(&mut t);
    TreeEncoder::new()
        .encode(
            &t,
            TreeEncoderOptions {
                init_cap: 1 << 10,
                max_cap: None,
            },
        )
        .unwrap()
}

fn check<'a>(name: &str, build: impl FnOnce(&mut TreeView<'a>)) -> Vec<u8> {
    let pkt = encode(build);
    assert_eq!(hex::encode(&pkt), expected(name), "{name}");
    pkt
}

#[test]
fn a_string_written_from_display_reads_back_where_it_lies() {
    let mut buf = [0xffu8; 128];
    let mut w = PacketWriter::new(&mut buf, 2);
    w.str_display(format_args!("{}-{}", "héllo", 42))
        .u32(Some(7));
    let n = w.finish().unwrap() as usize;
    let branch = TreeReader::new(&buf[..n]).unwrap().read_branch().unwrap();
    assert_eq!(branch.at(0).unwrap().get_str_ref().unwrap(), "héllo-42");
    assert!(
        branch.at(1).unwrap().get_str_ref().is_err(),
        "a number is not a string"
    );
    let mut small = [0u8; 40];
    let mut w = PacketWriter::new(&mut small, 1);
    w.str_display("far longer than the room that is left");
    assert_eq!(
        w.finish().map_err(|e| e.errnum),
        Err(Errno::EOVERFLOW),
        "a string that does not fit spoils the packet"
    );
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
    check("strs", |t| {
        t.strs(Some(&["a", "héllo", ""]));
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
        t.set(ViewValue::List(vec![
            ViewValue::I32(1),
            ViewValue::Str("x".into()),
        ]))
        .unwrap();
    });
    check("graft", |t| {
        t.graft(&inner).unwrap();
    });
}

#[test]
fn a_packet_under_the_cap_is_written_whatever_the_cap() {
    let bytes = [0u8; 1100];
    let mut t = TreeView::new();
    t.u8_array(Some(&bytes[..]));
    let options = TreeEncoderOptions {
        init_cap: 1 << 10,
        max_cap: Some(1500),
    };
    assert_eq!(
        TreeEncoder::new().encode(&t, options).unwrap().len(),
        16 + 8 + 1104
    );
}
