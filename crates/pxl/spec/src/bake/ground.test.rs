// pxl-spec/bake/ground.test.rs

use xpute_core::golden::Golden;

use super::*;
use crate::cell::cube::ground_pack;

const GOLDEN: &str = include_str!("../../golden/bake/ground.tsv");

#[test]
fn shown_ground_is_what_the_record_holds() {
    let v = Golden::load(GOLDEN);
    assert_eq!(BASE.type_id, v.u32("LAND_BASE_TYPE"));
    v.each("shown", |k| {
        let (mask, show) = (v.u32(&format!("{k}.mask")), v.bool(&format!("{k}.show")));
        // A mask bit names a channel by its identity, not by where it sits in
        // GROUND_CH_KEYS, so the records keep meaning the same cells when the
        // paint order moves.
        let read = |ch: GroundChannelKey| {
            let i = ch as u32;
            if (mask >> i) & 1 != 0 {
                10 + i
            } else {
                0
            }
        };
        let hidden = if show { 0 } else { channel_bit(GroundChannelKey::STRUCTURE) };
        let s = shown_ground(read, hidden);
        assert_eq!(
            (ground_pack::CHANNELS[s.channel as usize], s.type_id),
            (v.s(&format!("{k}.channel")), v.u32(&format!("{k}.type_id"))),
            "{k}"
        );
    });
}

#[test]
fn the_recess_is_the_types_whichever_channel_it_came_from() {
    let shown = |channel, type_id: u8| ShownGround { channel, type_id: type_id as u32 };
    assert_eq!(recess(shown(GroundChannelKey::WATER, cube_types::water::river)), 1);
    assert_eq!(recess(shown(GroundChannelKey::LAND, cube_types::land::sea)), 1);
    assert_eq!(recess(shown(GroundChannelKey::LAND, cube_types::land::bare)), 0);
    assert_eq!(recess(shown(GroundChannelKey::TRANSPORT, cube_types::water::river)), 0, "a transport type that shares river's number");
}
