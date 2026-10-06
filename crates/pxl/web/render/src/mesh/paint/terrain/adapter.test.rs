// pxl-render/mesh/paint/terrain/adapter.test.rs

use pxl_spec::bake::constant::ELE_PADDED_SPAN;
use pxl_spec::space::pyramid::{Tile, LEAF_Z};

use super::*;
use crate::mesh::run::apron_col;

/// A tile past the coverage gives no surface, so it puts no floor under
/// what is placed there.
#[test]
fn a_chunk_with_no_surface_has_no_ground_and_no_floor() {
    let cells = vec![sentinel::void; (TILE_CELLS * TILE_CELLS) as usize];
    let ele = vec![0i16; (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize];
    let decks = Decks::default();
    let tile = Tile::new(LEAF_Z, 0);
    let own = TileSource {
        tile: TileView {
            cells: &cells,
            ele: &ele,
            decks: &decks,
        },
        frame: Frame::of(tile),
        origin: [0, 0],
        n: 1,
    };
    let mut sample = ChunkSample::default();
    sample.read(&Around { own, beside: [None; 8] }, [0, 0]).unwrap();
    assert_eq!(sample.floor, i32::MIN);
    let i = apron_col(0, 0) as usize;
    assert!(!sample.has_ground(i));
    let mut runs = 0;
    sample.column_runs(i, |_, _, _, _| Ok::<(), ()>(runs += 1)).unwrap();
    assert_eq!(runs, 0);
}
