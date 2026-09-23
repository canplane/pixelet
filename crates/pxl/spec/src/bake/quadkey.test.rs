// pxl-spec/bake/quadkey.test.rs

use super::*;
use crate::bake::constant::{zoom_of_level, PAGE_TOP_LEVEL};

/// The page `script/smoke.ts` names and `space/xy.rs` measured in Bundang: its
/// lanes are (11569, 3671), which is 127.10°E 37.35°N, and the zoom-15 tile
/// there is (27953, 12712). Written out rather than derived, because a
/// derivation here would only repeat the arithmetic under test — and the
/// northward flip is the one thing in this file that can be wrong without
/// anything else noticing.
const BUNDANG: IVec = 12720250262323200;
const BUNDANG_QUADKEY: &str = "132110320312001";

#[test]
fn the_known_page_is_the_tile_it_stands_on() {
    assert_eq!(quadkey_of(LEVEL0_ZOOM, BUNDANG).to_string(), BUNDANG_QUADKEY);
    assert_eq!(index_of(BUNDANG_QUADKEY), Some((LEVEL0_ZOOM, BUNDANG)));
    assert_eq!(tile_of(BUNDANG, LEVEL0_ZOOM), (27953, 12712));
}

/// The property the name has this shape for: one prefix is one region at every
/// level it is held at.
#[test]
fn a_coarser_page_is_named_by_a_prefix_of_the_leaf_over_it() {
    let leaf = quadkey_of(LEVEL0_ZOOM, BUNDANG).to_string();
    for level in 1..=PAGE_TOP_LEVEL {
        let coarse = quadkey_of(zoom_of_level(level), ivec::sh(BUNDANG, -(level as i32))).to_string();
        assert_eq!(coarse.len(), (LEVEL0_ZOOM - level) as usize, "a level is its length");
        assert!(leaf.starts_with(&coarse), "level {level} {coarse} is not a prefix of {leaf}");
    }
}

#[test]
fn every_page_survives_the_round_trip_at_every_zoom() {
    let n = 1i64 << LEVEL0_ZOOM;
    for x in [-n / 2, -12345, -1, 0, 1, 11569, n / 2 - 1] {
        for y in [-n / 2, -7777, -1, 0, 1, 3671, n / 2 - 1] {
            for level in 0..=PAGE_TOP_LEVEL {
                let zoom = zoom_of_level(level);
                let idx = ivec::sh(ivec::gather(x, y, 0), -(level as i32));
                let key = quadkey_of(zoom, idx).to_string();
                assert_eq!(key.len(), zoom as usize);
                assert_eq!(index_of(&key), Some((zoom, idx)), "({x}, {y}) at level {level}");
            }
        }
    }
}

/// The X lane's two ends meet, so an index past the seam names the tile it
/// actually stands on rather than none at all.
#[test]
fn the_seam_wraps_rather_than_falling_outside() {
    let n = 1i64 << LEVEL0_ZOOM;
    let west = ivec::gather(-n / 2, 0, 0);
    assert_eq!(quadkey_of(LEVEL0_ZOOM, ivec::gather(n / 2, 0, 0)).to_string(), quadkey_of(LEVEL0_ZOOM, west).to_string());
}

#[test]
fn what_is_not_a_name_is_refused() {
    assert_eq!(index_of(""), None, "the empty string is no zoom");
    assert_eq!(index_of("4"), None, "a quadkey has four digits, not ten");
    assert_eq!(index_of("13211032031200a"), None, "nor any other letter");
    assert_eq!(index_of("-1321103203120"), None, "nor a sign: the digits carry it");
    assert_eq!(index_of(&"0".repeat(LEVEL0_ZOOM as usize + 1)), None, "nothing is finer than a leaf");
    assert!(index_of(&"0".repeat(LEVEL0_ZOOM as usize)).is_some(), "a leaf is");
}
