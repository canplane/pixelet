// xpute_core/io/marshal/tree/cursor.test.rs
// (no pair: the TypeScript tree has no test file)

use super::*;
use crate::io::marshal::tree::encode::{TreeEncoder, TreeEncoderOptions};
use crate::io::marshal::tree::spec::NodeValue;
use crate::io::marshal::tree::view::{NodeView, TreeView};
use crate::kit::array::Kind;
use crate::kit::encoding::{bytes_to_hex, hex_to_bytes};
use crate::vector::Vector;

const VECTOR: &str = include_str!("../../../../vector/io/marshal/tree/cursor.tsv");

fn typed_name(kind: Kind) -> &'static str {
    match kind {
        Kind::U8 => "Uint8Array",
        Kind::I8 => "Int8Array",
        Kind::U16 => "Uint16Array",
        Kind::I16 => "Int16Array",
        Kind::U32 => "Uint32Array",
        Kind::I32 => "Int32Array",
        Kind::F32 => "Float32Array",
        Kind::F64 => "Float64Array",
        Kind::U64 => "BigUint64Array",
        Kind::I64 => "BigInt64Array",
    }
}

/// A read value as the generator prints it.
fn show<T>(v: &NodeValue<T>) -> String {
    match v {
        NodeValue::Extra(_) => "branch".to_string(),
        NodeValue::Null => "null".to_string(),
        NodeValue::Bool(b) => format!("boolean:{b}"),
        NodeValue::Number(x) => format!("number:{x}"),
        NodeValue::BigInt(x) => format!("bigint:{x}"),
        NodeValue::Str(s) => format!("string:{}", bytes_to_hex(&crate::kit::encoding::te.encode(s))),
        NodeValue::List(l) => format!("[{}]", l.iter().map(show).collect::<Vec<_>>().join(",")),
        NodeValue::Array(a) => {
            let elements: Vec<String> = match a.kind {
                Kind::U8 => TypedArray::<u8>::of(a).unwrap().as_slice().iter().map(|x| x.to_string()).collect(),
                Kind::I8 => TypedArray::<i8>::of(a).unwrap().as_slice().iter().map(|x| x.to_string()).collect(),
                Kind::U16 => TypedArray::<u16>::of(a).unwrap().as_slice().iter().map(|x| x.to_string()).collect(),
                Kind::I16 => TypedArray::<i16>::of(a).unwrap().as_slice().iter().map(|x| x.to_string()).collect(),
                Kind::U32 => TypedArray::<u32>::of(a).unwrap().as_slice().iter().map(|x| x.to_string()).collect(),
                Kind::I32 => TypedArray::<i32>::of(a).unwrap().as_slice().iter().map(|x| x.to_string()).collect(),
                Kind::F32 => TypedArray::<f32>::of(a).unwrap().as_slice().iter().map(|x| (*x as f64).to_string()).collect(),
                Kind::F64 => TypedArray::<f64>::of(a).unwrap().as_slice().iter().map(|x| x.to_string()).collect(),
                Kind::U64 => TypedArray::<u64>::of(a).unwrap().as_slice().iter().map(|x| x.to_string()).collect(),
                Kind::I64 => TypedArray::<i64>::of(a).unwrap().as_slice().iter().map(|x| x.to_string()).collect(),
            };
            format!("{}({})", typed_name(a.kind), elements.join(" "))
        }
    }
}

fn aligned(hex: &str) -> U8Array {
    // A packet read at an 8-aligned offset, as the TypeScript's Uint8Array
    // over its own buffer is.
    let bytes = hex_to_bytes(hex);
    let out: U8Array = TypedArray::new(bytes.length());
    out.set(&bytes);
    out
}

#[test]
fn the_cursor_reads_what_the_typescript_reads() {
    let v = Vector::load(VECTOR);
    v.each("packets", |k| {
        let reader = TreeReader::new(aligned(v.s(&format!("{k}.packet")))).unwrap();
        let root = reader.read();
        assert_eq!(root.type_() as u32, v.u32(&format!("{k}.type")), "{k}.type");
        assert_eq!(show(&root.get_deep().unwrap()), v.s(&format!("{k}.deep")), "{k}.deep");
        if let Cursor::Branch(b) = &root {
            let got: Vec<String> = b
                .children()
                .unwrap()
                .iter()
                .map(|c| format!("{}:{}", c.type_(), if c.is_branch() { "branch".to_string() } else { show(&c.get().unwrap()) }))
                .collect();
            let n = if v.has(&format!("{k}.shallow.0")) { v.len(&format!("{k}.shallow")) } else { 0 };
            let want: Vec<&str> = (0..n).map(|j| v.s(&format!("{k}.shallow.{j}"))).collect();
            assert_eq!(got, want, "{k}.shallow");
        }
    });
    let good = v.s("packets.0.packet");
    let refused = |hex: &str| match TreeReader::new(aligned(hex)).and_then(|r| r.read().get_deep().map(|_| ())) {
        Ok(()) => "ok".to_string(),
        Err(e) => format!("errno:{}", e.errno as i32),
    };
    assert_eq!(refused(&good[..20]), v.s("malformed.truncated_header"));
    assert_eq!(refused(&format!("00{}", &good[2..])), v.s("malformed.bad_magic"));
    assert_eq!(refused(&good[..40]), v.s("malformed.truncated_payload"));
    v.each("huge", |k| assert_eq!(refused(v.s(&format!("{k}.packet"))), v.s(&format!("{k}.read")), "{k}"));
    for (j, n) in [0x7ffffff9u32, 0xfffffff8, 0xfffffff9].into_iter().enumerate() {
        let got = match crate::io::marshal::tree::spec::ALIGN(n, 8) {
            Ok(a) => format!("ok:{a}"),
            Err(e) => format!("errno:{}", e.errno as i32),
        };
        assert_eq!(got, v.s(&format!("align.{j}")), "ALIGN({n})");
    }

    let at_cap = |s: &str, cap: u32| {
        let mut t = TreeView::new();
        t.str(Some(s));
        match TreeEncoder::new().encode(
            &t,
            TreeEncoderOptions {
                max_cap: Some(cap),
                ..Default::default()
            },
        ) {
            Ok(p) => bytes_to_hex(&p),
            Err(e) => format!("errno:{}", e.errno as i32),
        }
    };
    for (j, cap) in [24, 32, 40, 48].into_iter().enumerate() {
        assert_eq!(at_cap("abcdefgh", cap), v.s(&format!("caps.{j}")), "caps.{j} at {cap}");
    }
    for (j, cap) in [24, 32, 40].into_iter().enumerate() {
        assert_eq!(at_cap("한글한", cap), v.s(&format!("caps_utf8.{j}")), "caps_utf8.{j} at {cap}");
    }
}
