// pxl-spec/bake/fold.test.rs

use super::*;

use crate::asset::cube::Ground;
use crate::ground::majority;
use crate::space::coord::zoom::LEAF_ZOOM;
use crate::space::layer::quadrant::{neighborhood, Quadrant};

const SPAN: usize = TILE_CELLS as usize;
const PADDED: usize = ELE_PADDED_SPAN as usize;

#[test]
fn area_majority_fold() {
    assert_eq!(majority([7, 7, 7, 1], GROUND.leaf_cell_zoom()), 7);
    assert_eq!(majority([7, 7, 1, 2], GROUND.leaf_cell_zoom()), 7);
    assert_eq!(majority([1, 7, 2, 7], GROUND.leaf_cell_zoom() - 1), 7);
    // No majority: the tie child, first under an odd step from the cell and last under an even one.
    assert_eq!(majority([7, 1, 2, 3], GROUND.leaf_cell_zoom() - 1), 7);
    assert_eq!(majority([7, 1, 2, 3], GROUND.leaf_cell_zoom() - 2), 3);
    assert_eq!(majority([7, 7, 1, 1], GROUND.leaf_cell_zoom() - 2), 1);
}

/// A leaf whose cell is `f` of it, its own four palette indices.
fn plane(f: impl Fn(usize, usize) -> u8) -> GroundCells {
    (0..SPAN * SPAN)
        .flat_map(|i| [f(i % SPAN, i / SPAN); 4])
        .collect::<Vec<_>>()
        .into()
}

/// Each parent cell's own palette index, as the zoom of the parent reads it.
fn fold(children: [&GroundCells; 4], zoom: Zoom) -> Vec<u8> {
    let four = fold_ground(children, zoom).to_vec();
    four.chunks(4)
        .map(|q| {
            own(
                u32::from_le_bytes([q[0], q[1], q[2], q[3]]),
                GROUND.cell_zoom(zoom),
            ) as u8
        })
        .collect()
}

#[test]
fn uniform_tile_fold() {
    let p = plane(|_, _| Ground::Subway as u8);
    assert!(fold([&p, &p, &p, &p], LEAF_ZOOM - 1)
        .iter()
        .all(|&v| v == Ground::Subway as u8));
}

#[test]
fn thin_area_tie() {
    // A one-cell strip ties 2:2 in every parent, so a leaf's parent's
    // south-west tie child keeps it only where x is even.
    let lot = plane(|x, _| if x == 10 { Ground::Parking as u8 } else { 0 });
    let empty = plane(|_, _| 0);
    assert_eq!(
        fold([&lot, &empty, &empty, &empty], LEAF_ZOOM - 1)[5],
        Ground::Parking as u8
    );
    let odd = plane(|x, _| if x == 11 { Ground::Parking as u8 } else { 0 });
    assert_eq!(fold([&odd, &empty, &empty, &empty], LEAF_ZOOM - 1)[5], 0);
    let dot = plane(|x, y| {
        if (x, y) == (10, 10) {
            Ground::Parking as u8
        } else {
            0
        }
    });
    assert!(fold([&dot, &empty, &empty, &empty], LEAF_ZOOM - 1)
        .iter()
        .all(|&v| v == 0));
}

#[test]
fn two_cell_feat_survives() {
    let road = plane(|x, _| if x == 10 || x == 11 { 3 } else { 0 });
    let empty = plane(|_, _| 0);
    let out = fold([&road, &empty, &empty, &empty], LEAF_ZOOM - 1);
    assert!((0..SPAN / 2).all(|py| out[py * SPAN + 5] == 3));
}

#[test]
fn child_quarters() {
    let q = [
        plane(|_, _| 1),
        plane(|_, _| 2),
        plane(|_, _| 3),
        plane(|_, _| 4),
    ];
    let out = fold([&q[0], &q[1], &q[2], &q[3]], 1);
    let half = SPAN / 2;
    assert_eq!(out[0], 1);
    assert_eq!(out[half], 2);
    assert_eq!(out[half * SPAN], 3);
    assert_eq!(out[half * SPAN + half], 4);
}

/// A padded grid whose sample at (i, j) is `f` of its world sample index.
fn grid(qx: i64, qy: i64, f: impl Fn(i64, i64) -> i64) -> Vec<i16> {
    let (inner, pad) = (TILE_ELE_SPAN as i64, ELE_PAD as i64);
    (0..PADDED * PADDED)
        .map(|s| {
            let (ix, iy) = ((s % PADDED) as i64 - pad, (s / PADDED) as i64 - pad);
            f(qx * inner + ix, qy * inner + iy) as i16
        })
        .collect()
}

fn fold_grids(f: impl Fn(i64, i64) -> i64 + Copy) -> Vec<i16> {
    fold_grids_of(f, |_| true)
}

/// The fold with only the tiles `has` says are held.
fn fold_grids_of(f: impl Fn(i64, i64) -> i64 + Copy, has: impl Fn(usize) -> bool) -> Vec<i16> {
    let grids: Vec<Vec<i16>> = (0..16).map(|k| grid(k % 4 - 1, k / 4 - 1, f)).collect();
    let refs: [Option<&[i16]>; 16] = std::array::from_fn(|k| has(k).then(|| grids[k].as_slice()));
    let mut out = vec![0; PADDED * PADDED];
    fold_ele(refs, &mut out);
    out
}

/// Folding from the four children alone matches the bake bit for bit except
/// the outermost padding sample.
#[test]
fn ele_fold_padding() {
    let f = |x: i64, y: i64| 7 * x - 3 * y + 11;
    let whole = fold_grids(f);
    let four = fold_grids_of(f, |k| [5, 6, 9, 10].contains(&k));
    let rim = |s: usize| s < ELE_PAD as usize - 1 || s >= PADDED - (ELE_PAD as usize - 1);
    let mut outer = 0;
    for sy in 0..PADDED {
        for sx in 0..PADDED {
            let at = sy * PADDED + sx;
            if rim(sx) || rim(sy) {
                outer += (whole[at] != four[at]) as u32;
            } else {
                assert_eq!(whole[at], four[at], "({sx}, {sy}) is the bake's");
            }
        }
    }
    assert!(outer > 0, "a ramp does run off the block's edge");
}

#[test]
fn ele_center_sample() {
    // A ramp of height 2m folds to 4i + 1 exactly: the parent's center in child samples.
    let out = fold_grids(|mx, _| 2 * mx);
    let pad = ELE_PAD as i64;
    for (sx, &v) in out.iter().take(PADDED).enumerate() {
        let i = sx as i64 - pad;
        assert_eq!(v as i64, 4 * i + 1, "sample {i}");
    }
}

#[test]
fn ele_skirt() {
    // The outermost padding lies four child samples past the block, in the neighbors.
    let out = fold_grids(|mx, my| {
        100 * mx.div_euclid(TILE_ELE_SPAN as i64) + my.div_euclid(TILE_ELE_SPAN as i64)
    });
    assert_eq!(out[0], -101);
    assert_eq!(out[PADDED - 1], 200 - 1);
    assert_eq!(out[(PADDED - 1) * PADDED], -100 + 2);
}

#[test]
fn ele_round_half_even() {
    // Rounding half up would drift every fold upward.
    let lows = fold_grids(|mx, _| mx.rem_euclid(2));
    assert!(
        lows.iter().all(|&v| v == 0),
        "0, 1, 0, 1 average to 0.5 and round to 0"
    );
    let highs = fold_grids(|mx, _| 1 + mx.rem_euclid(2) * 2);
    assert!(highs.iter().all(|&v| v == 2), "1, 3, 1, 3 average to 2");
    let halves = fold_grids(|mx, _| 2 + mx.rem_euclid(2));
    assert!(
        halves.iter().all(|&v| v == 2),
        "2, 3, 2, 3 average to 2.5 and round to 2"
    );
}

#[test]
fn volume_majority_fold() {
    let (a, b, empty) = (5, 9, 0);
    assert_eq!(
        majority(
            [a, empty, empty, empty, empty, empty, empty, empty],
            GROUND.leaf_cell_zoom() - 1
        ),
        empty
    );
    assert_eq!(
        majority(
            [a, a, a, a, empty, empty, empty, empty],
            GROUND.leaf_cell_zoom() - 1
        ),
        a
    );
    assert_eq!(
        majority(
            [empty, a, a, a, a, a, empty, empty],
            GROUND.leaf_cell_zoom() - 2
        ),
        a
    );
    let halves = [a, b, a, b, b, a, b, a];
    assert_eq!(majority(halves, GROUND.leaf_cell_zoom() - 1), halves[0]);
    assert_eq!(majority(halves, GROUND.leaf_cell_zoom() - 2), halves[7]);
    assert_eq!(
        majority(halves, GROUND.leaf_cell_zoom() + 1),
        halves[0],
        "a side finer than the cell alternates the same way"
    );
}

/// The open ocean: flat water at sea level, no tile.
fn zero_tile() -> TerrainTile {
    let cells = vec![Ground::Water as u8; 4 * SPAN * SPAN];
    TerrainTile::of(
        vec![0; PADDED * PADDED],
        cells.into(),
        Default::default(),
        Occupancy::ZERO,
    )
}

/// Zero tiles fold to the zero tile.
#[test]
fn zero_tile_fold() {
    let zero = zero_tile();
    assert_eq!(
        fold_terrain(
            neighborhood(Quadrant::new(LEAF_ZOOM - 1, [0, 0])).map(|_| Some(&zero)),
            LEAF_ZOOM - 1
        )
        .as_ref(),
        Some(&zero)
    );
}
