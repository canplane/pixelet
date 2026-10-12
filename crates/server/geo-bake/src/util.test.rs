// pxl-geo-bake/util.test.rs

use super::*;

fn tile() -> Vec<u8> {
    (0..100_000u32).map(|i| (i / 700) as u8).collect()
}

#[test]
fn a_packed_tile_unpacks_to_itself() {
    let packed = pack(&tile(), Level::Stored).unwrap();
    assert_eq!(unpack(&packed).unwrap(), tile());
}
