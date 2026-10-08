// pxl-geo-bake/util.test.rs

use super::*;

fn tile() -> Vec<u8> {
    (0..100_000u32).map(|i| (i / 700) as u8).collect()
}

#[test]
fn a_packed_tile_is_labeled_what_it_is_and_unpacks_to_itself() {
    let packed = pack(&tile(), Level::Stored).unwrap();
    assert!(is_packed(&packed));
    assert_eq!(unpack(&packed).unwrap(), tile());
}
