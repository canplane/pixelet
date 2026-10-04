// pxl-geo-bake/page/rekey.test.rs

use super::*;

/// The group four levels above is named by the page's first eleven digits.
const PAGE: &str = "12720250262323200";
const PAGE_QUADKEY: &str = "132110320312001";
const GROUP: &str = "794946921889792";
const GROUP_QUADKEY: &str = "13211032031";

#[test]
fn a_leaf_and_a_level_above_it_land_on_their_tiles() {
    for prefix in ["terrain/v6", "poi/v8"] {
        let pyramid = Bake::of(prefix).expect(prefix);
        assert_eq!(pyramid.rename(&format!("0/{PAGE}")), Ok(PAGE_QUADKEY.to_string()));
        assert_eq!(pyramid.rename(&format!("4/{GROUP}")), Ok(GROUP_QUADKEY.to_string()));
    }
    assert!(PAGE_QUADKEY.starts_with(GROUP_QUADKEY));
}

#[test]
fn a_bake_with_no_levels_is_named_at_its_own_zoom() {
    assert_eq!(Bake::of("geocode/v3").expect("geocode").rename(GROUP), Ok(GROUP_QUADKEY.to_string()));
}

#[test]
fn what_is_named_rather_than_keyed_is_carried_across() {
    assert_eq!(Bake::of("terrain/v5").expect("terrain").rename("_ocean"), Ok("_ocean".to_string()));
    assert_eq!(Bake::of("geocode/v3").expect("geocode").rename("_dict"), Ok("_dict".to_string()));
}

#[test]
fn what_cannot_be_read_is_an_error_rather_than_a_skip() {
    let terrain = Bake::of("terrain/v5").expect("terrain");
    assert!(terrain.rename(PAGE).is_err(), "a terrain page without its level");
    assert!(terrain.rename(&format!("99/{PAGE}")).is_err(), "a level past the pyramid");
    assert!(terrain.rename("0/not-a-number").is_err());
    assert!(Bake::of("asset").is_err(), "assets are named by their content and are not renamed");
}
