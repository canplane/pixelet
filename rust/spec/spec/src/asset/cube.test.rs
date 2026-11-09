// pxl-spec/asset/cube.test.rs

use super::*;
use crate::asset::decor::DECORS;

#[test]
fn cube_round_trip() {
    for &id in ALL_CUBES.iter() {
        assert_eq!(cube_of(id), Some(id));
        assert!(cube_rgb(id).is_some(), "{id}");
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
    for d in DECORS.iter() {
        assert_eq!(cube_of(d.id), None, "{}", d.id);
    }
}
