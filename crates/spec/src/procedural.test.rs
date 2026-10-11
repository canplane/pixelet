// pxl-spec/procedural.test.rs

use super::*;

#[test]
fn a_pier_stands_on_the_ground_and_not_where_the_terrain_gives_none() {
    use crate::block::cube::{ground, sentinel};
    assert_eq!(pier_stands([0, 10], ground::grass, [], 0, 1.0), Some([0, 10]));
    assert_eq!(pier_stands([0, 10], sentinel::unknown, [], 0, 1.0), None);
    assert_eq!(pier_stands([0, 10], sentinel::void, [], 0, 1.0), None);
}

// A square per seed of one to three cells, as a kind of mixed variants claims.
fn side(c: &Candidate) -> i64 {
    1 + (c.seed % 3) as i64
}

const REGION: [[i64; 2]; 2] = [[1000, 2000], [1064, 2064]];

#[test]
fn no_two_candidates_that_stand_claim_one_cell() {
    let mut out = Vec::new();
    scatter(7, 0.2, LEAF_ZOOM, REGION, 3, side, &mut out).unwrap();
    assert!(out.len() > 50, "{}", out.len());
    for (i, a) in out.iter().enumerate() {
        for b in &out[i + 1..] {
            assert!(!meet(a, b, &side), "{a:?} {b:?}");
        }
    }
}

#[test]
fn a_region_keeps_on_each_cell_what_the_cell_alone_keeps() {
    let mut out = Vec::new();
    scatter(7, 0.2, LEAF_ZOOM, REGION, 3, side, &mut out).unwrap();
    for y in REGION[0][1]..REGION[1][1] {
        for x in REGION[0][0]..REGION[1][0] {
            let at = out.iter().find(|c| c.cell == [x, y]).copied();
            assert_eq!(at, scatter_at(7, 0.2, [x, y], 3, side), "{x}, {y}");
        }
    }
}

#[test]
fn a_finer_zoom_keeps_what_a_coarser_one_had() {
    let (mut coarse, mut fine) = (Vec::new(), Vec::new());
    scatter(7, 0.2, LEAF_ZOOM - 2, REGION, 3, side, &mut coarse).unwrap();
    scatter(7, 0.2, LEAF_ZOOM, REGION, 3, side, &mut fine).unwrap();
    assert!(!coarse.is_empty());
    for c in &coarse {
        assert!(fine.contains(c), "{c:?}");
    }
}
