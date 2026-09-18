// xpute-core/ds/lattice_hash.test.rs
// (no pair: the hashes are tested beside the spatial hash, and this holds
// them to the TypeScript over signed addresses)

use super::*;
use crate::golden::Golden;

const GOLDEN: &str = include_str!("../../golden/ds/lattice_hash.tsv");

#[test]
fn lattice_hashes_are_the_typescript_s() {
    let v = Golden::load(GOLDEN);
    v.each("", |k| {
        let (x, y, z) = (v.i32(&format!("{k}.x")), v.i32(&format!("{k}.y")), v.i32(&format!("{k}.z")));
        assert_eq!(lattice_hash3(x, y, z), v.u32(&format!("{k}.h3")), "{k}.h3");
        assert_eq!(lattice_hash4(x, y, z, (x & 7) as u32), v.u32(&format!("{k}.h4")), "{k}.h4");
    });
}
