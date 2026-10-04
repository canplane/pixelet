// pxl-spec/bake/quadkey.test.rs

use super::*;
use crate::space::pyramid::{zoom_of_level, TOP_LEVEL};

/// A Bundang page, (11569, 3671), whose zoom-15 tile is (27953, 12712); written
/// out because the northward flip is what can be silently wrong.
const BUNDANG: IVec = 12720250262323200;
const BUNDANG_QUADKEY: &str = "132110320312001";

#[test]
fn the_known_page_is_the_tile_it_stands_on() {
    assert_eq!(quadkey_of(PAGE_ZOOM, BUNDANG).to_string(), BUNDANG_QUADKEY);
    assert_eq!(index_of(BUNDANG_QUADKEY), Some((PAGE_ZOOM, BUNDANG)));
    assert_eq!(tile_of(BUNDANG, PAGE_ZOOM), (27953, 12712));
}

/// One prefix is one region at every level.
#[test]
fn a_coarser_page_is_named_by_a_prefix_of_the_leaf_over_it() {
    let leaf = quadkey_of(PAGE_ZOOM, BUNDANG).to_string();
    for level in 1..=TOP_LEVEL {
        let coarse = quadkey_of(zoom_of_level(level), ivec::sh(BUNDANG, -(level as i32))).to_string();
        assert_eq!(coarse.len(), (PAGE_ZOOM - level) as usize, "a level is its length");
        assert!(leaf.starts_with(&coarse), "level {level} {coarse} is not a prefix of {leaf}");
    }
}

#[test]
fn every_page_survives_the_round_trip_at_every_zoom() {
    let n = 1i64 << PAGE_ZOOM;
    for x in [-n / 2, -12345, -1, 0, 1, 11569, n / 2 - 1] {
        for y in [-n / 2, -7777, -1, 0, 1, 3671, n / 2 - 1] {
            for level in 0..=TOP_LEVEL {
                let zoom = zoom_of_level(level);
                let idx = ivec::sh(ivec::gather(x, y, 0), -(level as i32));
                let key = quadkey_of(zoom, idx).to_string();
                assert_eq!(key.len(), zoom as usize);
                assert_eq!(index_of(&key), Some((zoom, idx)), "({x}, {y}) at level {level}");
            }
        }
    }
}

/// An index past the seam names the tile it stands on.
#[test]
fn the_seam_wraps_rather_than_falling_outside() {
    let n = 1i64 << PAGE_ZOOM;
    let west = ivec::gather(-n / 2, 0, 0);
    assert_eq!(quadkey_of(PAGE_ZOOM, ivec::gather(n / 2, 0, 0)).to_string(), quadkey_of(PAGE_ZOOM, west).to_string());
}

#[test]
fn what_is_not_a_name_is_refused() {
    assert_eq!(index_of(""), None, "the empty string is no zoom");
    assert_eq!(index_of("4"), None, "a quadkey has four digits, not ten");
    assert_eq!(index_of("13211032031200a"), None, "nor any other letter");
    assert_eq!(index_of("-1321103203120"), None, "nor a sign: the digits carry it");
    assert_eq!(index_of(&"0".repeat(PAGE_ZOOM as usize + 1)), None, "nothing is finer than a leaf");
    assert!(index_of(&"0".repeat(PAGE_ZOOM as usize)).is_some(), "a leaf is");
}

/// A name round-trips at every pyramid level, and a name no level holds is no page.
#[test]
fn a_page_s_name_reads_back_as_the_page_and_only_a_held_level_is_one() {
    for level in 0..=TOP_LEVEL {
        let p = LevelPage::new(level, ivec::gather(-3, 5, 0));
        assert_eq!(page_of_name(&page_name(p).to_string()), Some(p), "level {level}");
    }
    assert_eq!(page_of_name("0"), None, "zoom 1 is past the top level");
}
