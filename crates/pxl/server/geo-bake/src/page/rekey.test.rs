// pxl-geo-bake/page/rekey.test.rs

use super::*;

/// The group four zooms coarser is named by the page's first eleven digits.
const PAGE: &str = "12720250262323200";
const PAGE_QUADKEY: &str = "132110320312001";
const GROUP: &str = "794946921889792";
const GROUP_QUADKEY: &str = "13211032031";

#[test]
fn a_leaf_and_a_zoom_coarser_land_on_their_tiles() {
    assert_eq!(rename(&format!("z15/{PAGE}")), Ok(PAGE_QUADKEY.to_string()));
    assert_eq!(rename(&format!("z11/{GROUP}")), Ok(GROUP_QUADKEY.to_string()));
    assert!(PAGE_QUADKEY.starts_with(GROUP_QUADKEY));
}

#[test]
fn what_cannot_be_read_is_an_error_rather_than_a_skip() {
    assert!(rename(PAGE).is_err(), "a page without its zoom");
    assert!(rename(&format!("z2/{PAGE}")).is_err(), "a zoom past the pyramid");
    assert!(rename(&format!("15/{PAGE}")).is_err(), "a zoom is named as one");
    assert!(rename("z15/not-a-number").is_err());
    assert!(of("geocode").is_err(), "geocode is named by its quadkey as it is baked");
    assert!(of("asset").is_err(), "assets are named by their content and are not renamed");
}
