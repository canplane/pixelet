// pxl-geo-bake/terrain.test.rs

use super::*;
use pxl_spec::bake::separation::Separation;
use pxl_spec::bake::tile::terrain::{ELE_COLUMN, PALETTE_COLUMN};

/// A tile whose lines hold `points` points, as one line.
fn tile_of(points: usize) -> TerrainTile {
    let separation = Separation {
        starts: vec![0, points as u32].into(),
        points: vec![[0; 2]; points].into(),
        tops: vec![0; points].into(),
        widths: vec![1].into(),
        kinds: vec![0].into(),
        forms: vec![0].into(),
        clears: vec![0].into(),
        first_zooms: vec![0; points].into(),
    };
    TerrainTile::of(
        vec![0; ELE_COLUMN],
        vec![0; PALETTE_COLUMN].into(),
        separation,
        Occupancy::LEAF,
    )
}

/// A packet past the limit is not baked, and the refusal says its tile and its size.
#[test]
fn pkt_past_cap_refused() {
    let at = Quadrant::of_key("010332102301230").unwrap();
    let inside = tile_of(1000);
    assert_eq!(
        inside.encode(at).unwrap().len(),
        terrain::encode(&inside).unwrap().len()
    );

    let past = tile_of(400_000);
    let n = terrain::encode(&past).unwrap().len();
    assert!(n > 2 << 20, "the test's packet is past the limit");
    assert_eq!(
        past.encode(at).unwrap_err(),
        format!("tile 010332102301230 size {n} > 2097152")
    );
}
