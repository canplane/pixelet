// xpute-core/kit/rng.test.rs
// (no pair: rng.ts has no test file)

use super::*;
use crate::sys::vector::Vector;

const VECTOR: &str = include_str!("../../vector/kit/rng.tsv");

#[test]
fn rng_computes_what_the_typescript_computes() {
    let v = Vector::load(VECTOR);
    v.each("", |k| {
        let seed = v.u64(&format!("{k}.seed"));
        assert_eq!(mix64(seed), v.u64(&format!("{k}.mix64")), "{k}.mix64");
        assert_eq!(derive_seed_u64(seed, 77), v.u64(&format!("{k}.derive")), "{k}.derive");
        let mut rng = make_rng(Some(seed));
        let mut stream = SplitMix64::new(seed);
        v.each(&format!("{k}.draws"), |d| {
            let got = rng();
            assert_eq!(got, v.u32(d), "{d}");
            assert_eq!(stream.next_u32(), got, "{d}: the value draws the boxed stream's");
            let j = d.rsplit('.').next().unwrap();
            assert_eq!(u32_to_unit(got).to_bits(), v.f64(&format!("{k}.unit.{j}")).to_bits(), "{k}.unit.{j}");
        });
    });
}
