// pxl-spec/cell/object.test.rs

use super::*;
use crate::cell::cube::{cube_types, ground_pack};

/// A cube spelled as the shell spells it, `"land.bare"`: the words the
/// record kept, from the catalog's name tables.
fn spelled(cube: PlaceableCube) -> String {
    let (channel, t) = placeable_cube_parts(cube);
    let name = channel.map_or("block", |c| ground_pack::CHANNELS[c as usize]);
    let entries = cube_types::CHANNELS.iter().find(|(n, _)| *n == name).unwrap().1;
    format!("{name}.{}", entries.iter().find(|(_, x)| *x == t).unwrap().0)
}

#[test]
fn every_flat_color_round_trips_through_its_leaf_value_and_has_a_color() {
    for block in block_cubes() {
        let cube = PlaceableCube::Block(block);
        assert_eq!(placeable_cube_num_id(cube), block as u32);
        assert_eq!(placeable_cube_of(block as u32), Some(cube));
        assert_eq!(terrain_style_cube_source(cube), None);
        assert!(placeable_cube_rgb(cube).is_some(), "{}", spelled(cube));
    }
}

#[test]
fn a_cube_named_by_its_channel_and_number_is_the_cube() {
    for &cube in placeable_cubes.iter() {
        let (channel, t) = placeable_cube_parts(cube);
        assert_eq!(placeable_cube(channel, t as u32), Some(cube), "{}", spelled(cube));
    }
    assert_eq!(placeable_cube(None, cube_types::common::air as u32), None, "air is no flat color");
    assert_eq!(placeable_cube(Some(GroundChannelKey::WATER), cube_types::common::unknown as u32), None, "unknown is no water cube");
}

#[test]
fn a_terrain_styled_cube_round_trips_through_its_leaf_value() {
    let cube = terrain_style_cube_for(GroundChannelKey::LAND, cube_types::land::green_deep as u32).unwrap();
    assert_eq!(cube, PlaceableCube::Land(cube_types::Land::green_deep));
    assert_eq!(placeable_cube_of(placeable_cube_num_id(cube)), Some(cube));
    assert!(placeable_cube_rgb(cube).is_some());
}
