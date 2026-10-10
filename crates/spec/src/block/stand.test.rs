// pxl-spec/block/stand.test.rs

use crate::bake::constant::ELE_PADDED_SPAN;
use crate::bake::separation::Separation;
use crate::block::cube::{ground, object};
use crate::block::sample::{apron_col, Around, TileSource, TileView, COL_SPAN};
use crate::procedural::Frame;
use crate::space::layer::lattice::TILE_CELLS;
use crate::space::layer::tile::{Tile, LEAF_ZOOM};

use super::*;

/// Ground dug deeper than the chunk's floor is still drawn under the hole:
/// the floor goes under what is placed.
#[test]
fn ground_under_a_hole_past_the_floor_is_drawn() {
    let cells = vec![ground::land; (TILE_CELLS * TILE_CELLS) as usize];
    let ele = vec![5i16; (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize];
    let separation = Separation::default();
    let own = TileSource {
        tile: TileView {
            cells: &cells,
            ele: &ele,
            separation: &separation,
        },
        frame: Frame::of(Tile::new(LEAF_ZOOM, 0)),
        origin: [0, 0],
        n: 1,
    };
    let mut sample = ChunkSample::of_span(COL_SPAN);
    sample.read(&Around { own, beside: [None; 8] }, [0, 0], COL_SPAN).unwrap();
    let dug: HashMap<[i64; 3], u32> = (-5..5).map(|z| ([0, 0, z], object::air as u32)).collect();
    let mut stands = Vec::new();
    stands_of(&mut sample, &dug, (0, 0), GROUND.cell(), &mut stands).unwrap();
    let i = apron_col(COL_SPAN, 0, 0) as usize;
    let mut top = None;
    compose(&sample, &stands[..], i, sample.floor, |_, hi, block| {
        if let Block::Ground(_) = block {
            top = top.max(Some(hi));
        }
        Ok::<_, ()>(())
    })
    .unwrap();
    assert_eq!(top, Some(-6));
}
