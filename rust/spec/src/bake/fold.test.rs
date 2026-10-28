// pxl-spec/bake/fold.test.rs

use super::*;

use crate::asset::cube::ground;
use crate::space::layer::tile::{neighborhood, Tile};
use pxl_spec_core::param::tile::LEAF_ZOOM;
use pxl_spec_core::ground::majority;

const SPAN: usize = TILE_CELLS as usize;
const PADDED: usize = ELE_PADDED_SPAN as usize;

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

/// A leaf whose cell is `f` of it, its own four looks.
fn plane(f: impl Fn(usize, usize) -> u8) -> Ground {
    Ground {
        cells: (0..SPAN * SPAN)
            .flat_map(|i| [f(i % SPAN, i / SPAN); 4])
            .collect::<Vec<_>>()
            .into(),
    }
}

/// Each parent cell's own look, as the zoom of the parent reads it.
fn fold(children: [&Ground; 4], zoom: Zoom) -> Vec<u8> {
    let four = fold_ground(children, zoom).cells.to_vec();
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
fn a_uniform_tile_folds_to_itself() {
    let p = plane(|_, _| ground::subway);
    assert!(fold([&p, &p, &p, &p], LEAF_ZOOM - 1)
        .iter()
        .all(|&v| v == ground::subway));
}

#[test]
fn an_area_one_cell_wide_is_a_tie_and_a_dot_goes() {
    // A one-cell strip ties 2:2 in every parent, so a leaf's parent's
    // south-west tie child keeps it only where x is even.
    let lot = plane(|x, _| if x == 10 { ground::parking } else { 0 });
    let empty = plane(|_, _| 0);
    assert_eq!(
        fold([&lot, &empty, &empty, &empty], LEAF_ZOOM - 1)[5],
        ground::parking
    );
    let odd = plane(|x, _| if x == 11 { ground::parking } else { 0 });
    assert_eq!(fold([&odd, &empty, &empty, &empty], LEAF_ZOOM - 1)[5], 0);
    let dot = plane(|x, y| {
        if (x, y) == (10, 10) {
            ground::parking
        } else {
            0
        }
    });
    assert!(fold([&dot, &empty, &empty, &empty], LEAF_ZOOM - 1)
        .iter()
        .all(|&v| v == 0));
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
    fold_grids_held(f, |_| true)
}

/// The fold with only the tiles `held` says are held.
fn fold_grids_held(f: impl Fn(i64, i64) -> i64 + Copy, held: impl Fn(usize) -> bool) -> Vec<i16> {
    let grids: Vec<Vec<i16>> = (0..16).map(|k| grid(k % 4 - 1, k / 4 - 1, f)).collect();
    let refs: [Option<&[i16]>; 16] = std::array::from_fn(|k| held(k).then(|| grids[k].as_slice()));
    let mut out = vec![0; PADDED * PADDED];
    fold_ele(refs, &mut out);
    out
}

/// Folding from the four children alone matches the bake bit for bit except
/// the outermost padding sample.
#[test]
fn elevation_folded_from_the_four_children_differs_only_in_its_outermost_padding() {
    let f = |x: i64, y: i64| 7 * x - 3 * y + 11;
    let whole = fold_grids(f);
    let four = fold_grids_held(f, |k| [5, 6, 9, 10].contains(&k));
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
fn elevation_keeps_the_center_sample_convention() {
    // A ramp of height 2m folds to 4i + 1 exactly: the parent's center in child samples.
    let out = fold_grids(|mx, _| 2 * mx);
    let pad = ELE_PAD as i64;
    for (sx, &v) in out.iter().take(PADDED).enumerate() {
        let i = sx as i64 - pad;
        assert_eq!(v as i64, 4 * i + 1, "sample {i}");
    }
}

#[test]
fn elevation_reads_the_skirt_from_the_neighbors() {
    // The outermost padding lies four child samples past the block, in the neighbors.
    let out = fold_grids(|mx, my| {
        100 * mx.div_euclid(TILE_ELE_SPAN as i64) + my.div_euclid(TILE_ELE_SPAN as i64)
    });
    assert_eq!(out[0], -101);
    assert_eq!(out[PADDED - 1], 200 - 1);
    assert_eq!(out[(PADDED - 1) * PADDED], -100 + 2);
}

#[test]
fn elevation_rounds_its_halves_to_even() {
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
fn a_volume_keeps_what_fills_half_its_eight_and_loses_what_fills_less() {
    let (a, b, empty) = (5, 9, 0);
    assert_eq!(
        majority(
            [a, empty, empty, empty, empty, empty, empty, empty],
            GROUND.cell() - 1
        ),
        empty
    );
    assert_eq!(
        majority([a, a, a, a, empty, empty, empty, empty], GROUND.cell() - 1),
        a
    );
    assert_eq!(
        majority([empty, a, a, a, a, a, empty, empty], GROUND.cell() - 2),
        a
    );
    let halves = [a, b, a, b, b, a, b, a];
    assert_eq!(majority(halves, GROUND.cell() - 1), halves[0]);
    assert_eq!(majority(halves, GROUND.cell() - 2), halves[7]);
    assert_eq!(
        majority(halves, GROUND.cell() + 1),
        halves[0],
        "a side finer than the cell alternates the same way"
    );
}

/// The open ocean: flat water at sea level, no tile.
fn zero_tile() -> TerrainTile {
    let cells = vec![ground::water; 4 * SPAN * SPAN];
    TerrainTile::of(
        vec![0; PADDED * PADDED],
        Ground {
            cells: cells.into(),
        },
        Default::default(),
        Occupancy::ZERO,
    )
}

/// Zero tiles fold to the zero tile.
#[test]
fn the_zero_tile_folds_to_itself() {
    let zero = zero_tile();
    assert_eq!(
        fold_terrain(
            neighborhood(Tile::new(LEAF_ZOOM - 1, [0, 0])).map(|_| Some(&zero)),
            LEAF_ZOOM - 1
        )
        .as_ref(),
        Some(&zero)
    );
}
