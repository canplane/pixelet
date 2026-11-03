// pxl-spec/asset.test.rs

use super::*;
use crate::asset::cube::{object, sentinel, ALL_CUBES};
use crate::asset::id::{asset_id, Kind};

#[test]
fn asset_of_the_catalog() {
    for &id in ALL_CUBES.iter() {
        assert_eq!(Asset::of(id), Asset::Cube(id));
    }
    for d in decor::DECORS.iter() {
        assert_eq!(Asset::of(d.id), Asset::Decor(d));
    }
    assert_eq!(Asset::of(marker::MARKER), Asset::Marker);
}

/// One reading for every kind: what the catalog does not give out, another
/// version's among them, is unknown and stops nothing.
#[test]
fn asset_of_past_the_catalog() {
    let past = (1 << id::SHAPE_BITS) - 1;
    for id in [
        sentinel::void as AssetId,
        sentinel::unknown as AssetId,
        object::air as AssetId,
        asset_id(Kind::cube, past),
        asset_id(Kind::decor, past),
        asset_id(Kind::actor, 0),
        asset_id(Kind::sky, 0),
        u32::MAX,
    ] {
        assert_eq!(Asset::of(id), Asset::Unknown, "{id}");
    }
}
