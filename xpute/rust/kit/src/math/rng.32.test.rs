// xpute-kit/math/rng.32.test.rs

use super::*;
use crate::golden::Golden;
use crate::math::rng::u32_to_unit;

const GOLDEN: &str = include_str!("../../../../spec/golden/math/rng.32.tsv");

#[test]
fn rng32_computes_what_the_record_holds() {
    let v = Golden::load(GOLDEN);
    v.each("", |k| {
        let seed = v.u32(&format!("{k}.seed"));
        assert_eq!(mix32(seed), v.u32(&format!("{k}.mix32")), "{k}.mix32");
        assert_eq!(derive_seed_u32(seed, 77), v.u32(&format!("{k}.derive")), "{k}.derive");
        let mut stream = SplitMix32::new(seed);
        v.each(&format!("{k}.draws"), |d| {
            let got = stream.next_u32();
            assert_eq!(got, v.u32(d), "{d}");
            let j = d.rsplit('.').next().unwrap();
            assert_eq!(u32_to_unit(got).to_bits(), v.f64(&format!("{k}.unit.{j}")).to_bits(), "{k}.unit.{j}");
        });
    });
}
