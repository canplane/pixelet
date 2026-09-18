// pxl-spec/bake/ground.test.rs
// (no pair: ground.ts has no test file)

use xpute_core::golden::Golden;

use super::*;

const GOLDEN: &str = include_str!("../../golden/bake/ground.tsv");

#[test]
fn shown_ground_is_what_the_typescript_shows() {
    let v = Golden::load(GOLDEN);
    assert_eq!(LAND_BASE_TYPE, v.u32("LAND_BASE_TYPE"));
    v.each("shown", |k| {
        let (mask, show) = (v.u32(&format!("{k}.mask")), v.bool(&format!("{k}.show")));
        let read = |ch: GroundChannelKey| {
            let i = GROUND_CH_KEYS.iter().position(|c| *c == ch).unwrap() as u32;
            if (mask >> i) & 1 != 0 {
                10 + i
            } else {
                0
            }
        };
        let s = shown_ground(read, show);
        assert_eq!((s.channel.as_str(), s.type_id), (v.s(&format!("{k}.channel")), v.u32(&format!("{k}.type_id"))), "{k}");
    });
}
