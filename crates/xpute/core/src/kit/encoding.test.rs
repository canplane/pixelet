// xpute_core/kit/encoding.test.rs
// (no pair: encoding.ts has no test file)

use super::*;
use crate::vector::Vector;

const VECTOR: &str = include_str!("../../vector/kit/encoding.tsv");

/// A string field, which the flattening leaves out when it is empty.
fn text(v: &Vector, key: &str) -> &'static str {
    if v.has(key) {
        v.s(key)
    } else {
        ""
    }
}

#[test]
fn encoding_computes_what_the_typescript_computes() {
    let v = Vector::load(VECTOR);
    v.each("blobs", |k| {
        let b = hex_to_bytes(text(&v, &format!("{k}.hex")));
        let s = bytes_to_b64url(&b);
        assert_eq!(s, text(&v, &format!("{k}.b64url")), "{k}.b64url");
        assert_eq!(bytes_to_hex(&b64url_to_bytes(&s)), text(&v, &format!("{k}.back")), "{k}.back");
        assert_eq!(is_b64url(&s), v.bool(&format!("{k}.is")), "{k}.is");
        assert_eq!(bytes_to_hex(&hex_to_bytes(&bytes_to_hex(&b))), text(&v, &format!("{k}.hex_back")), "{k}.hex_back");
    });
    v.each("texts", |k| {
        let t = text(&v, &format!("{k}.t"));
        assert_eq!(bytes_to_hex(&te.encode(t)), text(&v, &format!("{k}.utf8")), "{k}.utf8");
        assert_eq!(td.decode(&te.encode(t)) == t, v.bool(&format!("{k}.back")), "{k}.back");
    });
    // A throw there is a panic here.
    v.each("bad_hex", |k| {
        let s = ["1g", "zz", "0G", " 1", "aB"][k.rsplit('.').next().unwrap().parse::<usize>().unwrap()];
        let got = std::panic::catch_unwind(|| bytes_to_hex(&hex_to_bytes(s))).map_or("throws".to_string(), |h| format!("ok:{h}"));
        assert_eq!(got, v.s(k), "hex_to_bytes({s:?})");
    });
    v.each("invalid", |k| {
        assert_eq!(is_b64url(v.s(&format!("{k}.s"))), v.bool(&format!("{k}.is")), "{k}");
    });
}
