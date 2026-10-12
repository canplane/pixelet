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

/// Caves are some of the ground, not none and not most.
#[test]
fn caves_hollow_a_little_of_the_ground() {
    let mut n = 0;
    for x in 0..64 {
        for y in 0..64 {
            for z in -32..0 {
                n += usize::from(hollow([x as f64 + 0.5, y as f64 + 0.5, z as f64 + 0.5]));
            }
        }
    }
    let share = n as f64 / (64.0 * 64.0 * 32.0);
    eprintln!("cave share {share}");
    assert!((0.005..0.1).contains(&share), "{share}");
}

/// The caves fold to themselves: at every zoom a cube is the majority of its
/// eight children.
#[test]
fn a_cave_cube_is_the_majority_of_its_children() {
    use crate::space::coord::zoom::units;
    for zoom in CAVE_COARSEST - 1..GROUND.cell() {
        let side = units(zoom);
        for x in 0..6 {
            for y in 0..6 {
                for z in -6..0 {
                    let corner = [x, y, z].map(|v| f64::from(v) * side);
                    let center = corner.map(|v| v + side / 2.0);
                    let hollow = (0..8)
                        .filter(|k| hollow_at(zoom + 1, [0, 1, 2].map(|a| center[a] + if k >> a & 1 == 1 { side / 4.0 } else { -side / 4.0 })))
                        .count();
                    assert_eq!(hollow > 4, hollow_at(zoom, center), "z{zoom} {corner:?}: {hollow} of 8");
                }
            }
        }
    }
}

/// Walking the drawing cubes finds every hollow cube the cubes say, and only
/// those, at a leaf's zoom and at a coarser one.
#[test]
fn the_caves_walked_are_the_cubes_that_are_hollow() {
    use crate::space::coord::zoom::units;
    use std::collections::BTreeSet;
    let (min, max) = ([-40.0, 8.0, -70.0], [24.0, 72.0, 2.0]);
    for zoom in [GROUND.cell(), GROUND.cell() - 2] {
        let side = units(zoom);
        let mut walked = BTreeSet::new();
        caves(min, max, zoom, &mut |z, c| {
            let n = 1i64 << (zoom - z);
            for x in 0..n {
                for y in 0..n {
                    for w in 0..n {
                        walked.insert([c[0] * n + x, c[1] * n + y, c[2] * n + w]);
                    }
                }
            }
        });
        let cells = |k: usize| (min[k] / side).floor() as i64..(max[k] / side).ceil() as i64;
        let mut hollow = BTreeSet::new();
        for x in cells(0) {
            for y in cells(1) {
                for z in cells(2) {
                    if hollow_at(zoom, [x, y, z].map(|v| (v as f64 + 0.5) * side)) {
                        hollow.insert([x, y, z]);
                    }
                }
            }
        }
        walked.retain(|c| (0..3).all(|k| cells(k).contains(&c[k])));
        assert!(!hollow.is_empty());
        assert_eq!(walked, hollow, "z{zoom}");
    }
}
