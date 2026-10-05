// pxl-spec/cell/object.test.rs

use super::*;
use crate::cell::cube::cube_types::{object, sentinel};

#[test]
fn every_placeable_cube_round_trips_through_its_leaf_value_and_has_a_color() {
    for &cube in placeable_cubes.iter() {
        assert_eq!(placeable_cube_of(placeable_cube_num_id(cube)), Some(cube));
        assert!(placeable_cube_rgb(cube).is_some(), "{cube:?}");
    }
}

#[test]
fn no_sentinel_air_or_decor_is_a_cube() {
    for value in [sentinel::void as u32, sentinel::unknown as u32, object::air as u32] {
        assert_eq!(placeable_cube_of(value), None, "{value}");
    }
    for &id in decor_ids.iter() {
        assert_eq!(placeable_cube_of(decor_num_id(id)), None, "{id:?}");
    }
}
