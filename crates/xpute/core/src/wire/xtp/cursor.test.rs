// xpute-core/wire/xtp/cursor.test.rs
// (no pair: the TypeScript tree has no test file)

use super::*;
use crate::golden::Golden;
use crate::wire::xtp::encode::{TreeEncoder, TreeEncoderOptions};
use crate::wire::xtp::spec::{NodeValue, SequenceType};
use crate::wire::xtp::view::{NodeView, TreeView};

const GOLDEN: &str = include_str!("../../../golden/wire/xtp/cursor.tsv");

/// A read value as the generator prints it, which means as JavaScript would:
/// every width the reader can hand back that a `number` carried prints
/// `number:`, and the two a `bigint` carried print `bigint:`.
fn show<T>(v: &NodeValue<T>) -> String {
    match v {
        NodeValue::Extra(_) => "branch".to_string(),
        NodeValue::Null => "null".to_string(),
        NodeValue::Bool(b) => format!("boolean:{b}"),
        NodeValue::U8(x) => format!("number:{x}"),
        NodeValue::I8(x) => format!("number:{x}"),
        NodeValue::U16(x) => format!("number:{x}"),
        NodeValue::I16(x) => format!("number:{x}"),
        NodeValue::U32(x) => format!("number:{x}"),
        NodeValue::I32(x) => format!("number:{x}"),
        NodeValue::U64(x) => format!("bigint:{x}"),
        NodeValue::I64(x) => format!("bigint:{x}"),
        NodeValue::F32(x) => format!("number:{}", *x as f64),
        NodeValue::F64(x) => format!("number:{x}"),
        NodeValue::Str(s) => format!("string:{}", hex::encode(s.as_bytes())),
        NodeValue::List(l) => format!("[{}]", l.iter().map(show).collect::<Vec<_>>().join(",")),
        NodeValue::Array(a) => {
            let b: &[u8] = &a.bytes;
            let (name, elements): (&str, Vec<String>) = match a.type_ {
                SequenceType::U8_ARRAY => ("Uint8Array", b.iter().map(|x| x.to_string()).collect()),
                SequenceType::BITSET => ("Uint8Array", b.iter().map(|x| x.to_string()).collect()),
                SequenceType::I8_ARRAY => ("Int8Array", b.iter().map(|x| (*x as i8).to_string()).collect()),
                SequenceType::U16_ARRAY => ("Uint16Array", b.chunks(2).map(|c| u16::from_le_bytes(c.try_into().unwrap()).to_string()).collect()),
                SequenceType::I16_ARRAY => ("Int16Array", b.chunks(2).map(|c| i16::from_le_bytes(c.try_into().unwrap()).to_string()).collect()),
                SequenceType::U32_ARRAY => ("Uint32Array", b.chunks(4).map(|c| u32::from_le_bytes(c.try_into().unwrap()).to_string()).collect()),
                SequenceType::I32_ARRAY => ("Int32Array", b.chunks(4).map(|c| i32::from_le_bytes(c.try_into().unwrap()).to_string()).collect()),
                SequenceType::U64_ARRAY => ("BigUint64Array", b.chunks(8).map(|c| u64::from_le_bytes(c.try_into().unwrap()).to_string()).collect()),
                SequenceType::I64_ARRAY => ("BigInt64Array", b.chunks(8).map(|c| i64::from_le_bytes(c.try_into().unwrap()).to_string()).collect()),
                SequenceType::F32_ARRAY => ("Float32Array", b.chunks(4).map(|c| (f32::from_le_bytes(c.try_into().unwrap()) as f64).to_string()).collect()),
                SequenceType::F64_ARRAY => ("Float64Array", b.chunks(8).map(|c| f64::from_le_bytes(c.try_into().unwrap()).to_string()).collect()),
                SequenceType::STR => unreachable!("a string leaf reads as NodeValue::Str"),
            };
            format!("{name}({})", elements.join(" "))
        }
    }
}

/// A packet at a word-aligned address, which is what `TreeReader` asks of one
/// so its f64 lanes can be viewed in place. A `Vec<u8>` is aligned to a byte,
/// so the bytes are laid past the first aligned index of a larger one.
struct Packet {
    buf: Vec<u8>,
    at: usize,
    len: usize,
}

impl Packet {
    fn new(hex: &str) -> Packet {
        let bytes = hex::decode(hex).unwrap();
        let mut buf = vec![0u8; bytes.len() + WORD_SZ as usize];
        let at = buf.as_ptr().align_offset(WORD_SZ as usize);
        buf[at..at + bytes.len()].copy_from_slice(&bytes);
        Packet { buf, at, len: bytes.len() }
    }

    fn as_slice(&self) -> &[u8] {
        &self.buf[self.at..self.at + self.len]
    }
}

#[test]
fn the_cursor_reads_what_the_typescript_reads() {
    let v = Golden::load(GOLDEN);
    v.each("packets", |k| {
        let pkt = Packet::new(v.s(&format!("{k}.packet")));
        let reader = TreeReader::new(pkt.as_slice()).unwrap();
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
    let refused = |hex: &str| {
        let pkt = Packet::new(hex);
        match TreeReader::new(pkt.as_slice()).and_then(|r| r.read().get_deep().map(|_| ())) {
            Ok(()) => "ok".to_string(),
            Err(e) => format!("errno:{}", e.errno as i32),
        }
    };
    assert_eq!(refused(&good[..20]), v.s("malformed.truncated_header"));
    assert_eq!(refused(&format!("00{}", &good[2..])), v.s("malformed.bad_magic"));
    assert_eq!(refused(&good[..40]), v.s("malformed.truncated_payload"));
    v.each("huge", |k| assert_eq!(refused(v.s(&format!("{k}.packet"))), v.s(&format!("{k}.read")), "{k}"));
    for (j, n) in [0x7ffffff9u32, 0xfffffff8, 0xfffffff9].into_iter().enumerate() {
        let got = match crate::wire::xtp::spec::ALIGN(n, 8) {
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
            Ok(p) => hex::encode(&p),
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
