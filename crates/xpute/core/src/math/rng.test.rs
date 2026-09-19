// xpute-core/math/rng.test.rs
// (no pair: rng.ts has no test file)

use super::*;
use crate::golden::Golden;

const GOLDEN: &str = include_str!("../../golden/math/rng.tsv");

#[test]
fn rng_computes_what_the_record_holds() {
    let v = Golden::load(GOLDEN);
    v.each("", |k| {
        let seed = v.u64(&format!("{k}.seed"));
        assert_eq!(mix64(seed), v.u64(&format!("{k}.mix64")), "{k}.mix64");
        assert_eq!(derive_seed_u64(seed, 77), v.u64(&format!("{k}.derive")), "{k}.derive");
        let mut stream = SplitMix64::new(seed);
        v.each(&format!("{k}.draws"), |d| {
            let got = stream.next_u32();
            assert_eq!(got, v.u32(d), "{d}");
            let j = d.rsplit('.').next().unwrap();
            assert_eq!(u32_to_unit(got).to_bits(), v.f64(&format!("{k}.unit.{j}")).to_bits(), "{k}.unit.{j}");
        });
    });
}
