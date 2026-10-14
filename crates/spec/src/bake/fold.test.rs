// pxl-spec/bake/fold.test.rs

use super::*;

use crate::bake::tile::terrain::HEIGHT_LANE;
use crate::block::cube::ground;
use crate::space::layer::tile::LEAF_ZOOM;
use pxl_portable::ground::majority;

const SPAN: usize = TILE_CELLS as usize;

#[test]
fn an_area_keeps_what_fills_half_its_four_and_ties_alternate_corners() {
    assert_eq!(majority([7, 7, 7, 1], GROUND.cell()), 7);
    assert_eq!(majority([7, 7, 1, 2], GROUND.cell()), 7);
    assert_eq!(majority([1, 7, 2, 7], GROUND.cell() - 1), 7);
    // No majority: the tie child, first under an odd step from the cell and last under an even one.
    assert_eq!(majority([7, 1, 2, 3], GROUND.cell() - 1), 7);
    assert_eq!(majority([7, 1, 2, 3], GROUND.cell() - 2), 3);
    assert_eq!(majority([7, 7, 1, 1], GROUND.cell() - 2), 1);
}

/// A tile whose cell is `f` of it.
fn plane(f: impl Fn(usize, usize) -> u8) -> Ground {
    Ground {
        cells: (0..SPAN * SPAN).map(|i| f(i % SPAN, i / SPAN)).collect::<Vec<_>>().into(),
    }
}

fn fold(children: [&Ground; 4], zoom: Zoom) -> Vec<u8> {
    let tiles = children.map(|g| TerrainTile::leaf(vec![0; HEIGHT_LANE], g.clone(), Default::default()));
    fold_terrain(tiles.each_ref().map(Some), zoom).unwrap().ground.cells.to_vec()
}

#[test]
fn a_uniform_tile_folds_to_itself() {
    let p = plane(|_, _| ground::subway);
    assert!(fold([&p, &p, &p, &p], LEAF_ZOOM - 1).iter().all(|&v| v == ground::subway));
}

#[test]
fn an_area_one_cell_wide_is_a_tie_and_a_dot_goes() {
    // A one-cell strip ties 2:2 in every parent, so a leaf's parent's
    // south-west tie child keeps it only where x is even.
    let lot = plane(|x, _| if x == 10 { ground::parking } else { 0 });
    let empty = plane(|_, _| 0);
    assert_eq!(fold([&lot, &empty, &empty, &empty], LEAF_ZOOM - 1)[5], ground::parking);
    let odd = plane(|x, _| if x == 11 { ground::parking } else { 0 });
    assert_eq!(fold([&odd, &empty, &empty, &empty], LEAF_ZOOM - 1)[5], 0);
    let dot = plane(|x, y| if (x, y) == (10, 10) { ground::parking } else { 0 });
    assert!(fold([&dot, &empty, &empty, &empty], LEAF_ZOOM - 1).iter().all(|&v| v == 0));
}

#[test]
fn a_feature_two_cells_wide_survives_a_fold() {
    let road = plane(|x, _| if x == 10 || x == 11 { 3 } else { 0 });
    let empty = plane(|_, _| 0);
    let out = fold([&road, &empty, &empty, &empty], LEAF_ZOOM - 1);
    assert!((0..SPAN / 2).all(|py| out[py * SPAN + 5] == 3));
}

#[test]
fn each_child_lands_in_its_own_quarter() {
    let q = [plane(|_, _| 1), plane(|_, _| 2), plane(|_, _| 3), plane(|_, _| 4)];
    let out = fold([&q[0], &q[1], &q[2], &q[3]], 1);
    let half = SPAN / 2;
    assert_eq!(out[0], 1);
    assert_eq!(out[half], 2);
    assert_eq!(out[half * SPAN], 3);
    assert_eq!(out[half * SPAN + half], 4);
}

/// A padded grid whose sample at (i, j) is `f` of its world sample index.
/// A child of grounded cells whose height is `f` of the cell, and one of
/// their looks where `void` says.
fn heights(f: impl Fn(usize, usize) -> i16, void: impl Fn(usize, usize) -> bool) -> TerrainTile {
    let cells = plane(|x, y| if void(x, y) { 0 } else { ground::grass });
    TerrainTile::leaf((0..SPAN * SPAN).map(|i| f(i % SPAN, i / SPAN)).collect(), cells, Default::default())
}

/// A cell is the rounded mean of the grounded heights of its four, and
/// lies within their lowest and highest.
#[test]
fn a_height_is_its_grounded_fours_mean_rounded() {
    let t = heights(|x, y| (x % 2 * 2 + y % 2 * 5) as i16, |_, _| false);
    let out = fold_terrain([Some(&t), Some(&t), Some(&t), Some(&t)], LEAF_ZOOM - 1).unwrap();
    assert!(out.heights.iter().all(|&h| h == 4), "0, 2, 5, 7 average to 3.5 and round to 4");
    let one = heights(|x, _| if x % 2 == 0 { 9 } else { 100 }, |x, _| x % 2 == 1);
    let out = fold_terrain([Some(&one), Some(&one), Some(&one), Some(&one)], LEAF_ZOOM - 1).unwrap();
    assert!(out.heights.iter().all(|&h| h == 9), "the void cells' heights are no ground's");
}

#[test]
fn a_volume_keeps_what_fills_half_its_eight_and_loses_what_fills_less() {
    let (a, b, empty) = (5, 9, 0);
    assert_eq!(majority([a, empty, empty, empty, empty, empty, empty, empty], GROUND.cell() - 1), empty);
    assert_eq!(majority([a, a, a, a, empty, empty, empty, empty], GROUND.cell() - 1), a);
    assert_eq!(majority([empty, a, a, a, a, a, empty, empty], GROUND.cell() - 2), a);
    let halves = [a, b, a, b, b, a, b, a];
    assert_eq!(majority(halves, GROUND.cell() - 1), halves[0]);
    assert_eq!(majority(halves, GROUND.cell() - 2), halves[7]);
    assert_eq!(majority(halves, GROUND.cell() + 1), halves[0], "a side finer than the cell alternates the same way");
}

/// The open ocean: flat water at sea level, no tile.
fn zero_tile() -> TerrainTile {
    let cells = vec![ground::water; SPAN * SPAN];
    TerrainTile::of(vec![0; HEIGHT_LANE], Ground { cells: cells.into() }, Default::default(), Occupancy::ZERO)
}

/// Four zero tiles fold to the zero tile, levels included.
#[test]
fn the_zero_tile_folds_to_itself() {
    let zero = zero_tile();
    assert_eq!(fold_terrain([Some(&zero); 4], LEAF_ZOOM - 1).as_ref(), Some(&zero));
}
