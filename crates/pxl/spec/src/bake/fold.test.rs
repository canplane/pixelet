// pxl-spec/bake/fold.test.rs

use super::*;

const SPAN: usize = PAGE_CELL_SPAN as usize;
const PADDED: usize = ELE_PADDED_SPAN as usize;

#[test]
fn majority_takes_half_and_ties_take_the_given_child() {
    assert_eq!(majority([7, 7, 7, 7], 0), 7);
    assert_eq!(majority([7, 7, 7, 1], 0), 7);
    assert_eq!(majority([7, 7, 1, 2], 0), 7);
    assert_eq!(majority([7, 1, 7, 2], 0), 7);
    assert_eq!(majority([7, 7, 1, 1], 9), 9);
    assert_eq!(majority([7, 1, 2, 3], 9), 9);
}

fn plane(f: impl Fn(usize, usize) -> u8) -> Vec<u8> {
    (0..SPAN * SPAN).map(|i| f(i % SPAN, i / SPAN)).collect()
}

fn fold(children: [&[u8]; 4], level: u32) -> Vec<u8> {
    let mut out = vec![0; SPAN * SPAN];
    fold_plane(children, level, &mut out);
    out
}

#[test]
fn a_uniform_page_folds_to_itself() {
    let p = plane(|_, _| 11);
    assert!(fold([&p, &p, &p, &p], 1).iter().all(|&v| v == 11));
}

#[test]
fn a_one_cell_line_is_a_tie_and_a_dot_goes() {
    // A one-cell road down x = 10: every parent cell it touches holds it in
    // two of four children — a tie, taken by the corner child, which on level
    // 1 is the south-west one and so sees the road only where x is even.
    let road = plane(|x, _| if x == 10 { 3 } else { 0 });
    let empty = plane(|_, _| 0);
    let out = fold([&road, &empty, &empty, &empty], 1);
    assert_eq!(out[5], 3);
    let odd = plane(|x, _| if x == 11 { 3 } else { 0 });
    let out = fold([&odd, &empty, &empty, &empty], 1);
    assert_eq!(out[5], 0);
    // A single cell is one of four and always goes.
    let dot = plane(|x, y| if (x, y) == (10, 10) { 3 } else { 0 });
    assert!(fold([&dot, &empty, &empty, &empty], 1).iter().all(|&v| v == 0));
}

#[test]
fn a_feature_two_cells_wide_survives_a_fold() {
    let road = plane(|x, _| if x == 10 || x == 11 { 3 } else { 0 });
    let empty = plane(|_, _| 0);
    let out = fold([&road, &empty, &empty, &empty], 1);
    assert!((0..SPAN / 2).all(|py| out[py * SPAN + 5] == 3));
}

#[test]
fn ties_alternate_their_corner_by_level() {
    // Columns tie every block 2:2 with the two corners on opposite sides, so
    // the parent is whichever corner the level names: (0, 0) on odd levels,
    // (1, 1) on even ones.
    let columns = plane(|x, _| if x % 2 == 0 { 1 } else { 2 });
    assert!(fold([&columns, &columns, &columns, &columns], 1).iter().all(|&v| v == 1));
    assert!(fold([&columns, &columns, &columns, &columns], 2).iter().all(|&v| v == 2));
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

/// A padded elevation grid whose sample at inner index (i, j) is `f` of that
/// sample's index across the whole world, page `q` being `q · PAGE_ELE_SPAN`
/// samples along.
fn grid(qx: i64, qy: i64, f: impl Fn(i64, i64) -> i64) -> Vec<i16> {
    let (inner, pad) = (PAGE_ELE_SPAN as i64, ELE_PAD as i64);
    (0..PADDED * PADDED)
        .map(|s| {
            let (ix, iy) = ((s % PADDED) as i64 - pad, (s / PADDED) as i64 - pad);
            f(qx * inner + ix, qy * inner + iy) as i16
        })
        .collect()
}

fn fold_grids(f: impl Fn(i64, i64) -> i64 + Copy) -> Vec<i16> {
    let grids: Vec<Vec<i16>> = (0..16).map(|k| grid(k % 4 - 1, k / 4 - 1, f)).collect();
    let refs: [&[i16]; 16] = std::array::from_fn(|k| grids[k].as_slice());
    let mut out = vec![0; PADDED * PADDED];
    fold_ele(refs, &mut out);
    out
}

#[test]
fn elevation_keeps_the_centre_sample_convention() {
    // Child sample m sits at m + 0.5; the parent's sample i covers children 2i
    // and 2i + 1, whose centres average to 2i + 1 — the parent's own centre in
    // child samples. So a ramp of height 2m folds to 4i + 1 exactly.
    let out = fold_grids(|mx, _| 2 * mx);
    let pad = ELE_PAD as i64;
    for (sx, &v) in out.iter().take(PADDED).enumerate() {
        let i = sx as i64 - pad;
        assert_eq!(v as i64, 4 * i + 1, "sample {i}");
    }
}

#[test]
fn elevation_reads_the_skirt_from_the_neighbours() {
    // The parent's outermost padding samples lie four child samples past the
    // block, which only the neighbouring pages hold. Heights that differ by
    // page show which page each came from.
    let out = fold_grids(|mx, my| 100 * mx.div_euclid(PAGE_ELE_SPAN as i64) + my.div_euclid(PAGE_ELE_SPAN as i64));
    assert_eq!(out[0], -101);
    assert_eq!(out[PADDED - 1], 200 - 1);
    assert_eq!(out[(PADDED - 1) * PADDED], -100 + 2);
}

#[test]
fn elevation_rounds_its_halves_to_even() {
    // Four integers average to a half a quarter of the time; rounding half up
    // would drift every fold upward.
    let lows = fold_grids(|mx, _| mx.rem_euclid(2));
    assert!(lows.iter().all(|&v| v == 0), "0, 1, 0, 1 average to 0.5 and round to 0");
    let highs = fold_grids(|mx, _| 1 + mx.rem_euclid(2) * 2);
    assert!(highs.iter().all(|&v| v == 2), "1, 3, 1, 3 average to 2");
    let halves = fold_grids(|mx, _| 2 + mx.rem_euclid(2));
    assert!(halves.iter().all(|&v| v == 2), "2, 3, 2, 3 average to 2.5 and round to 2");
}

#[test]
fn a_volume_keeps_what_fills_half_its_eight_and_loses_what_fills_less() {
    let (a, b, empty) = (5, 9, 0);
    // A lone cube is an eighth: it goes.
    assert_eq!(majority8([a, empty, empty, empty, empty, empty, empty, empty], 1), empty);
    // A slab — the lower four — is half: it stays.
    assert_eq!(majority8([a, a, a, a, empty, empty, empty, empty], 1), a);
    // Five of eight win whatever the tie child holds.
    assert_eq!(majority8([empty, a, a, a, a, a, empty, empty], 2), a);
    // Two halves tie, and the tie alternates by level between the diagonal's ends.
    let halves = [a, b, a, b, b, a, b, a];
    assert_eq!(majority8(halves, 1), halves[0]);
    assert_eq!(majority8(halves, 2), halves[7]);
    assert_eq!(majority8(halves, -1), halves[0], "a level below the cell alternates the same way");
}
