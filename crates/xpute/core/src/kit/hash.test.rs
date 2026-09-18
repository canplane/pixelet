// xpute-core/kit/hash.test.rs
// (no pair: hash.ts has no test file)

use super::*;
use crate::golden::Golden;
use crate::kit::encoding::hex_to_bytes;

const GOLDEN: &str = include_str!("../../golden/kit/hash.tsv");

#[test]
fn hash_computes_what_the_typescript_computes() {
    let v = Golden::load(GOLDEN);
    v.each("strings", |k| {
        assert_eq!(fnv1a64_str(v.s(&format!("{k}.s"))), v.u64(&format!("{k}.fnv")), "{k}");
    });
    // The empty string flattens to no `s` line; its hash is the offset basis.
    v.each("bytes", |k| {
        let b = if v.has(&format!("{k}.b")) { hex_to_bytes(v.s(&format!("{k}.b"))) } else { hex_to_bytes("") };
        assert_eq!(fnv1a_64_hex(&b), v.s(&format!("{k}.hex")), "{k}");
    });
}
