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
        Sentinel::Void as u32,
        Sentinel::Unknown as u32,
        Sentinel::Air as u32,
    ] {
        assert_eq!(cube_of(val), None, "{val}");
    }
    for d in DECORS.iter() {
        assert_eq!(cube_of(d.id), None, "{}", d.id);
    }
}

/// A body is stopped by what is a cube, in the catalog or not, and by
/// nothing else.
#[test]
fn what_blocks_a_body() {
    for id in [
        Sentinel::Void as u32,
        Sentinel::Air as u32,
        1 << IDX_BITS,
        2 << IDX_BITS,
    ] {
        assert!(!solid(id), "{id}");
    }
    for id in [
        1,
        Sentinel::Unknown as u32,
        Sentinel::Air as u32 + 1,
        (1 << IDX_BITS) - 1,
    ] {
        assert!(solid(id), "{id}");
    }
}
