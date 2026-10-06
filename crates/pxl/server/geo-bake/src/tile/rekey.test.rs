// pxl-geo-bake/tile/rekey.test.rs

use super::*;

/// The group four zooms coarser is named by the tile's first eleven digits.
const TILE: &str = "12720250262323200";
const TILE_QUADKEY: &str = "310332102130223";
const GROUP: &str = "794946921889792";
const GROUP_QUADKEY: &str = "31033210213";

#[test]
fn a_leaf_and_a_zoom_coarser_land_on_their_tiles() {
    assert_eq!(rename(&format!("z15/{TILE}")), Ok(TILE_QUADKEY.to_string()));
    assert_eq!(rename(&format!("z11/{GROUP}")), Ok(GROUP_QUADKEY.to_string()));
    assert!(TILE_QUADKEY.starts_with(GROUP_QUADKEY));
}

#[test]
fn what_cannot_be_read_is_an_error_rather_than_a_skip() {
    assert!(rename(TILE).is_err(), "a tile without its zoom");
    assert!(rename(&format!("z2/{TILE}")).is_err(), "a zoom past the pyramid");
    assert!(rename(&format!("15/{TILE}")).is_err(), "a zoom is named as one");
    assert!(rename("z15/not-a-number").is_err());
    assert!(of("geocode").is_err(), "geocode is named by its quadkey as it is baked");
    assert!(of("asset").is_err(), "assets are named by their content and are not renamed");
}
