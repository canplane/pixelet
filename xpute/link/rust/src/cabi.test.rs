// xpute-link/cabi.test.rs

use super::*;
use crate::abi::types::{Lent, Token};
use crate::{load_uri, store_uri};
use xpute_golden::Golden;

const GOLDEN: &str = include_str!("../../spec/golden/cabi.tsv");

/// The record's packet: one of each number, a string, a list of numbers, a
/// URI and a lent span, as the Canonical ABI lays a tuple of them.
const FIXED_SZ: usize = 80;

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
    Lent { at: 4096, len: 64 }.store(&mut w, 72);
    w.finish()
}

/// The packet lowers as the record holds it, and only into a buffer that holds
/// it whole.
#[test]
fn lower_matches_record() {
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
    assert_eq!(Lent::load(r, 72), Ok(Lent { at: 4096, len: 64 }));
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
