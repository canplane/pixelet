// pxl-core/domain/layer/object/asset.test.rs

use pxl_spec::asset::cube::{object, sentinel, ALL_CUBES};
use pxl_spec::asset::decor::DECORS;
use pxl_spec::asset::id::{Kind, SHAPE_BITS};
use pxl_spec::asset::marker::MARKER;

use super::*;

#[test]
fn of_the_catalog() {
    for &id in ALL_CUBES.iter() {
        assert_eq!(Asset::of(id), Asset::Cube(id));
    }
    for d in DECORS.iter() {
        assert_eq!(Asset::of(d.id), Asset::Decor(d.id));
        assert_eq!(Asset::of(d.id).decor(), Some(d));
    }
    assert_eq!(Asset::of(MARKER), Asset::Marker);
}

/// An id the catalog does not give out, one from another version's catalog
/// among them, reads as unknown and does not stop the core.
#[test]
fn past_the_catalog() {
    let shape = |kind: Kind, n: u32| (kind as u32) << SHAPE_BITS | n;
    let past = (1 << SHAPE_BITS) - 1;
    for id in [
        sentinel::void as AssetId,
        sentinel::unknown as AssetId,
        object::air as AssetId,
        shape(Kind::cube, past),
        shape(Kind::decor, past),
        // A kind the layout does not give out.
        3 << SHAPE_BITS,
        u32::MAX,
    ] {
        assert_eq!(Asset::of(id), Asset::Unknown, "{id}");
        assert_eq!(Asset::of(id).kind(), None, "{id}");
    }
}
