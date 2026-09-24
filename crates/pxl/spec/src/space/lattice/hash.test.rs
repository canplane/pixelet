// pxl-spec/space/lattice/hash.test.rs

// The hashes are tested beside the spatial hash; this holds them to the
// TypeScript's records over signed addresses.

use super::*;
use xpute_core::golden::Golden;

const GOLDEN: &str = include_str!("../../../golden/spec/space/lattice/hash.tsv");

#[test]
fn lattice_hashes_are_the_record_s() {
    let v = Golden::load(GOLDEN);
    v.each("", |k| {
        let (x, y, z) = (v.i32(&format!("{k}.x")), v.i32(&format!("{k}.y")), v.i32(&format!("{k}.z")));
        assert_eq!(hash3(x, y, z), v.u32(&format!("{k}.h3")), "{k}.h3");
        assert_eq!(hash4(x, y, z, (x & 7) as u32), v.u32(&format!("{k}.h4")), "{k}.h4");
    });
}
