// pxl-spec/asset/cube.test.rs

use super::*;
use crate::asset::{decor_num_id, DECOR_IDS};

#[test]
fn cube_round_trip() {
    for &cube in ALL_CUBES.iter() {
        assert_eq!(cube_of(cube_id(cube)), Some(cube));
        assert!(cube_rgb(cube).is_some(), "{cube:?}");
    }
}

#[test]
fn non_cubes() {
    for val in [
        sentinel::void as u32,
        sentinel::unknown as u32,
        object::air as u32,
    ] {
        assert_eq!(cube_of(val), None, "{val}");
    }
    for &id in DECOR_IDS.iter() {
        assert_eq!(cube_of(decor_num_id(id)), None, "{id:?}");
    }
}
