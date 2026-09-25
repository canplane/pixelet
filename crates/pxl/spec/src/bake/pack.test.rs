// pxl-spec/bake/pack.test.rs

use super::*;

use crate::cell::cube::cube_types;

#[test]
fn every_number_a_channel_gives_out_comes_back_as_itself() {
    for key in GROUND_CH_KEYS {
        for number in ground_pack::NUMBER[field_of(key)] {
            let cell = with_channel(0, key, *number);
            assert_eq!(channel_of(cell, key), *number, "{key:?} lost {number}");
        }
    }
}

#[test]
fn the_four_fields_do_not_reach_into_each_other() {
    // Every channel at its last entry at once: the widest word the pack can
    // write, and each field still answers for itself.
    let numbers = GROUND_CH_KEYS.map(|k| {
        let row = ground_pack::NUMBER[field_of(k)];
        row[row.len() - 2]
    });
    let cell = pack(numbers);
    assert_eq!(unpack(cell), numbers);
    // And one field written again moves nothing else.
    let before = unpack(with_channel(cell, GroundChannelKey::WATER, cube_types::water::lake));
    let after = GROUND_CH_KEYS.map(|k| {
        if k == GroundChannelKey::WATER {
            cube_types::water::lake
        } else {
            numbers[GROUND_CH_KEYS.iter().position(|c| c == &k).unwrap()]
        }
    });
    assert_eq!(before, after);
}
