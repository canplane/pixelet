// xpute-link/cabi.test.rs

//! Holds the Canonical ABI's lowering and lifting to link/spec/golden/cabi.tsv,
//! the record every language's must match, and writes it: run with
//! `XPUTE_GOLDEN_WRITE=1`, `lower_matches_record` writes the record from
//! `lower` and `REFUSED` instead of comparing.

use std::fmt::Write as _;

use super::*;
use crate::abi::types::{Lent, Token};
use crate::{load_uri, store_uri};
use xpute_golden::Golden;

const GOLDEN: &str = include_str!("../../spec/golden/cabi.tsv");

/// The record's packet: one of each number, a string, a list of numbers, a
/// URI and a lent buffer, as the Canonical ABI lays a tuple of them.
const FIXED_SZ: usize = 88;

/// Packets that do not hold a URI, each refused as one: a list whose
/// pointer, count or token runs past the packet, and a token of no case or a
/// string past its bytes.
const REFUSED: [&str; 6] = [
    "0c000000",
    "0a000000000000000000",
    "0800000002000000000000000000000000000000",
    "0800000001000000020000000500000000000000",
    "0800000001000000010000001400000001000000ff",
    "080000000100000001000000140000000500000066",
];

fn uri() -> Vec<Token> {
    vec![Token::Int(5), Token::Str("f".to_owned())]
}

fn lower(buf: &mut [u8]) -> Result<usize, Errno> {
    let mut w = Lower::new(buf, FIXED_SZ);
    w.bool(0, true);
    w.i8(1, -3);
    w.u16(2, 65535);
    w.i32(4, -7);
    w.u64(8, 1 << 63);
    w.f32(16, 0.25);
    w.f64(24, -1.5);
    w.u8(32, 200);
    w.i16(34, -2);
    w.u32(36, 4000000000);
    w.i64(40, -(1 << 40));
    w.str(48, "€€");
    w.arr::<u16>(56, &[1, 2, 65535]);
    store_uri(&mut w, 64, &uri());
    Lent {
        buf: 4096,
        nbyte: 64,
    }
    .store(&mut w, 72);
    w.finish()
}

/// The packet lowers as the record holds it, and only into a buffer that holds
/// it whole; with `XPUTE_GOLDEN_WRITE=1`, the record is written instead.
#[test]
fn lower_matches_record() {
    if std::env::var_os("XPUTE_GOLDEN_WRITE").is_some() {
        return write();
    }
    let want = Golden::load(GOLDEN).bytes("pkt");
    let mut buf = vec![0xaa; want.len()];
    assert_eq!(lower(&mut buf), Ok(want.len()));
    assert_eq!(hex::encode(&buf), hex::encode(&want));
    let mut short = vec![0; want.len() - 1];
    assert_eq!(lower(&mut short), Err(Errno::EMSGSIZE));
}

/// The record's packet lifts as the values it was lowered from.
#[test]
fn lift_matches_record() {
    let pkt = Golden::load(GOLDEN).bytes("pkt");
    let r = Lift::new(&pkt);
    assert_eq!(r.bool(0), Ok(true));
    assert_eq!(r.i8(1), Ok(-3));
    assert_eq!(r.u16(2), Ok(65535));
    assert_eq!(r.i32(4), Ok(-7));
    assert_eq!(r.u64(8), Ok(1 << 63));
    assert_eq!(r.f32(16), Ok(0.25));
    assert_eq!(r.f64(24), Ok(-1.5));
    assert_eq!(r.u8(32), Ok(200));
    assert_eq!(r.i16(34), Ok(-2));
    assert_eq!(r.u32(36), Ok(4000000000));
    assert_eq!(r.i64(40), Ok(-(1 << 40)));
    assert_eq!(r.str(48), Ok("€€"));
    assert_eq!(r.arr::<u16>(56), Ok(vec![1, 2, 65535]));
    assert_eq!(load_uri(r, 64), Ok(uri()));
    assert_eq!(
        Lent::load(r, 72),
        Ok(Lent {
            buf: 4096,
            nbyte: 64
        })
    );
}

/// Each recorded packet that does not hold a URI is refused as one, EBADMSG.
#[test]
fn refusals_match_record() {
    let g = Golden::load(GOLDEN);
    g.each("refused", |k| {
        assert_eq!(
            load_uri(Lift::new(&g.bytes(k)), 0),
            Err(Errno::EBADMSG),
            "{k}"
        );
    });
}

/// Writes the record: the packet `lower` lays, and the refused packets.
fn write() {
    let mut buf = vec![0; 4096];
    let n = lower(&mut buf).expect("the packet lowered");
    let mut out = format!("pkt\t{}\n", hex::encode(&buf[..n]));
    for (k, r) in REFUSED.iter().enumerate() {
        let _ = writeln!(out, "refused.{k}\t{r}");
    }
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../spec/golden/cabi.tsv");
    std::fs::write(path, out).expect("the record written");
}
