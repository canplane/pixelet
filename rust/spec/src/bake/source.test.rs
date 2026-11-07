// pxl-spec/bake/source.test.rs

use xpute_kit::golden::Golden;

use super::*;

/// The record the client's source is held to too
/// (ts/client/src/sys/fs/source.test.ts).
const GOLDEN: &str = include_str!("../../golden/bake/version.tsv");

#[test]
fn is_version_golden() {
    let g = Golden::load(GOLDEN);
    let mut cases = 0;
    while g.has(&format!("case.{cases}.version")) {
        let version = g.s(&format!("case.{cases}.version"));
        let want = g.bool(&format!("case.{cases}.is_version"));
        assert_eq!(is_version(version), want, "`{version}`");
        cases += 1;
    }
    assert!(cases > 0);
}
