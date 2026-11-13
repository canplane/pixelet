// xpute-kit/math/rng.test.rs

use super::*;

// ---- the 32-bit family ----

mod w32 {
    use super::*;
    use crate::math::rng::u32_to_unit;
    use xpute_golden::Golden;

    const GOLDEN: &str = include_str!("../../../spec/golden/math/rng.32.tsv");

    #[test]
    fn matches_record() {
        let v = Golden::load(GOLDEN);
        v.each("", |k| {
            let seed = v.u32(&format!("{k}.seed"));
            assert_eq!(mix32(seed), v.u32(&format!("{k}.mix32")), "{k}.mix32");
            assert_eq!(
                seed_mix32(seed, 77),
                v.u32(&format!("{k}.derive")),
                "{k}.derive"
            );
            let mut stream = SplitMix32::new(seed);
            v.each(&format!("{k}.draws"), |d| {
                let got = stream.next_u32();
                assert_eq!(got, v.u32(d), "{d}");
                let j = d.rsplit('.').next().unwrap();
                assert_eq!(
                    u32_to_unit(got).to_bits(),
                    v.f64(&format!("{k}.unit.{j}")).to_bits(),
                    "{k}.unit.{j}"
                );
            });
        });
    }
}

// ---- the 64-bit family ----

mod w64 {
    use super::*;
    use crate::math::rng::u32_to_unit;
    use xpute_golden::Golden;

    const GOLDEN: &str = include_str!("../../../spec/golden/math/rng.64.tsv");

    #[test]
    fn matches_record() {
        let v = Golden::load(GOLDEN);
        v.each("", |k| {
            let seed = v.u64(&format!("{k}.seed"));
            assert_eq!(mix64(seed), v.u64(&format!("{k}.mix64")), "{k}.mix64");
            assert_eq!(
                seed_mix64(seed, 77),
                v.u64(&format!("{k}.derive")),
                "{k}.derive"
            );
            let mut stream = SplitMix64::new(seed);
            v.each(&format!("{k}.draws"), |d| {
                let got = stream.next_u32();
                assert_eq!(got, v.u32(d), "{d}");
                let j = d.rsplit('.').next().unwrap();
                assert_eq!(
                    u32_to_unit(got).to_bits(),
                    v.f64(&format!("{k}.unit.{j}")).to_bits(),
                    "{k}.unit.{j}"
                );
            });
        });
    }
}
