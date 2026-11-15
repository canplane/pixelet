// pxl-geo-bake/terrain.test.rs

use super::*;
use pxl_spec::bake::separation::Separation;
use pxl_spec::bake::tile::Occupancy;
use pxl_spec::bake::tile::terrain::{ELE_COLUMN, FOLD_COLUMN};

/// A tile whose lines hold `points` points, as one line.
fn tile_of(points: usize) -> TerrainTile {
    let separation = Separation {
        starts: vec![0, points as u32],
        points: vec![[0; 2]; points],
        heights: vec![0; points],
        widths: vec![1],
        asset_ids: vec![0],
        forms: vec![0],
        clearances: vec![0],
        first_zooms: vec![0; points],
    };
    TerrainTile::of(
        vec![0; ELE_COLUMN],
        vec![0; FOLD_COLUMN].into(),
        separation,
        Occupancy::LEAF,
    )
}

/// A packet past the cap a client holds one to is not baked, and the refusal
/// says its tile.
#[test]
fn pkt_past_cap_refused() {
    let at = Quadrant::of_key("010332102301230").unwrap();
    let inside = tile_of(1000);
    assert_eq!(
        inside.encode(at).unwrap(),
        terrain::encode(&inside).unwrap()
    );

    let past = tile_of(400_000);
    assert!(
        past.encode(at)
            .unwrap_err()
            .starts_with("010332102301230: ")
    );
}
