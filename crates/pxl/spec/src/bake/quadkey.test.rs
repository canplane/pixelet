// pxl-spec/bake/quadkey.test.rs

use super::*;

/// A Bundang tile, (11569, 3671), whose grid place is (27953, 12712); written
/// out because the northward flip is what can be silently wrong.
const BUNDANG: IVec = 12720250262323200;
const BUNDANG_QUADKEY: &str = "132110320312001";

#[test]
fn the_known_tile_is_the_tile_it_stands_on() {
    assert_eq!(quadkey_of(TILE_KEY_DEPTH, BUNDANG).to_string(), BUNDANG_QUADKEY);
    assert_eq!(index_of(BUNDANG_QUADKEY), Some((TILE_KEY_DEPTH, BUNDANG)));
    assert_eq!(tile_of(BUNDANG, TILE_KEY_DEPTH), (27953, 12712));
}

/// One prefix is one region at every zoom.
#[test]
fn a_coarser_tile_is_named_by_a_prefix_of_the_leaf_over_it() {
    let leaf = quadkey_of(TILE_KEY_DEPTH, BUNDANG).to_string();
    assert_eq!(depth_at(LEAF_Z), TILE_KEY_DEPTH, "a tile of the leaves' zoom is a leaf");
    for z in TOP_Z..LEAF_Z {
        let coarse = tile_name(Tile::over(z, BUNDANG)).to_string();
        assert_eq!(coarse.len(), depth_at(z) as usize, "a zoom is its length");
        assert!(leaf.starts_with(&coarse), "z{z} {coarse} is not a prefix of {leaf}");
    }
}

#[test]
fn every_tile_survives_the_round_trip_at_every_zoom() {
    let n = 1i64 << TILE_KEY_DEPTH;
    for x in [-n / 2, -12345, -1, 0, 1, 11569, n / 2 - 1] {
        for y in [-n / 2, -7777, -1, 0, 1, 3671, n / 2 - 1] {
            for z in TOP_Z..=LEAF_Z {
                let depth = depth_at(z);
                let idx = Tile::over(z, ivec::gather(x, y, 0)).tile;
                let key = quadkey_of(depth, idx).to_string();
                assert_eq!(key.len(), depth as usize);
                assert_eq!(index_of(&key), Some((depth, idx)), "({x}, {y}) at z{z}");
            }
        }
    }
}

/// An index past the seam names the tile it stands on.
#[test]
fn the_seam_wraps_rather_than_falling_outside() {
    let n = 1i64 << TILE_KEY_DEPTH;
    let west = ivec::gather(-n / 2, 0, 0);
    assert_eq!(quadkey_of(TILE_KEY_DEPTH, ivec::gather(n / 2, 0, 0)).to_string(), quadkey_of(TILE_KEY_DEPTH, west).to_string());
}

#[test]
fn what_is_not_a_name_is_refused() {
    assert_eq!(index_of(""), None, "the empty string is no tile");
    assert_eq!(index_of("4"), None, "a quadkey has four digits, not ten");
    assert_eq!(index_of("13211032031200a"), None, "nor any other letter");
    assert_eq!(index_of("-1321103203120"), None, "nor a sign: the digits carry it");
    assert_eq!(index_of(&"0".repeat(TILE_KEY_DEPTH as usize + 1)), None, "nothing is finer than a leaf");
    assert!(index_of(&"0".repeat(TILE_KEY_DEPTH as usize)).is_some(), "a leaf is");
}

/// A name round-trips at every zoom of the pyramid, and a name no zoom holds is no tile.
#[test]
fn a_tile_s_name_reads_back_as_the_tile_and_only_a_held_zoom_is_one() {
    for z in TOP_Z..=LEAF_Z {
        let p = Tile::new(z, ivec::gather(-3, 5, 0));
        assert_eq!(tile_of_name(&tile_name(p).to_string()), Some(p), "z{z}");
    }
    assert_eq!(tile_of_name("0"), None, "a one-digit name is coarser than the top");
}
