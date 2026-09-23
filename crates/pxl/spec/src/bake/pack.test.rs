// pxl-spec/bake/pack.test.rs

use super::*;

use crate::bake::ground::shown_ground;
use crate::cell::cube::cube_types;

/// The four numbers in `GROUND_CH_KEYS`' order, as a page's planes hold them.
fn planes(land: u8, water: u8, transport: u8, structure: u8) -> [u8; 4] {
    GROUND_CH_KEYS.map(|k| match k {
        GroundChannelKey::LAND => land,
        GroundChannelKey::WATER => water,
        GroundChannelKey::TRANSPORT => transport,
        GroundChannelKey::STRUCTURE => structure,
    })
}

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

#[test]
fn a_cell_nothing_painted_is_a_word_of_zero() {
    assert_eq!(pack([cube_types::common::void; 4]), 0);
    assert_eq!(unpack(0), [cube_types::common::void; 4]);
}

#[test]
fn a_number_no_channel_gives_out_reads_as_unknown() {
    // 200 is in no channel's catalog. An unreadable byte in a plane already
    // meant UNKNOWN, and the pack keeps that rather than inventing a type.
    let cell = with_channel(0, GroundChannelKey::LAND, 200);
    assert_eq!(channel_of(cell, GroundChannelKey::LAND), cube_types::common::unknown);
}

#[test]
fn what_the_viewer_sees_is_what_the_planes_said() {
    // The whole point of keeping four fields: `shown_ground` reads a packed
    // cell exactly as it read four planes, hidden channels and all.
    let cases = [
        planes(cube_types::land::bare, 0, cube_types::transport::road_default, 0),
        planes(cube_types::land::residential, 0, cube_types::transport::road_primary, cube_types::structure::building),
        planes(cube_types::land::sea, cube_types::water::river, 0, 0),
    ];
    for numbers in cases {
        let cell = pack(numbers);
        for hidden in [0, crate::bake::ground::channel_bit(GroundChannelKey::STRUCTURE)] {
            let from_planes = shown_ground(|k| numbers[GROUND_CH_KEYS.iter().position(|c| c == &k).unwrap()] as u32, hidden);
            let from_cell = shown_ground(|k| channel_of(cell, k) as u32, hidden);
            assert_eq!(from_cell, from_planes, "{numbers:?} hidden {hidden}");
        }
    }
}

#[test]
fn the_fields_are_the_tables_and_nothing_wider() {
    for (f, row) in ground_pack::NUMBER.iter().enumerate() {
        let widest = (1u32 << ground_pack::BITS[f]) as usize;
        assert!(row.len() <= widest, "{} entries in {} bits", row.len(), ground_pack::BITS[f]);
        assert!(row.len() * 2 > widest, "{} entries would fit a narrower field", row.len());
    }
    assert_eq!(ground_pack::CELL_BITS, ground_pack::BITS.iter().sum::<u32>());
}
