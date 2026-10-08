// pxl-spec/block/object.test.rs

use super::*;
use crate::block::cube::{object, sentinel};

#[test]
fn every_cube_round_trips_through_its_leaf_value_and_has_a_color() {
    for &cube in ALL_CUBES.iter() {
        assert_eq!(cube_of(cube_id(cube)), Some(cube));
        assert!(cube_rgb(cube).is_some(), "{cube:?}");
    }
}

#[test]
fn no_sentinel_air_or_decor_is_a_cube() {
    for value in [sentinel::void as u32, sentinel::unknown as u32, object::air as u32] {
        assert_eq!(cube_of(value), None, "{value}");
    }
    for &id in decor_ids.iter() {
        assert_eq!(cube_of(decor_num_id(id)), None, "{id:?}");
    }
}
