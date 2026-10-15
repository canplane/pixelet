// pxl-spec/procedural.test.rs

use super::*;
use crate::block::DecorId;
use crate::space::layer::lattice::GROUND;
use crate::space::layer::tile::LEAF_ZOOM;

#[test]
fn a_pier_stands_on_the_ground_and_not_where_the_terrain_gives_none() {
    use crate::block::cube::{ground, sentinel};
    assert_eq!(pier_stands([0, 10], ground::grass, [], 0, 1.0), Some([0, 10]));
    assert_eq!(pier_stands([0, 10], sentinel::unknown, [], 0, 1.0), None);
    assert_eq!(pier_stands([0, 10], sentinel::void, [], 0, 1.0), None);
}

// Boats: a kind of mixed variants, whose squares are two to four cells.
fn boat() -> &'static Decor {
    crate::block::decor::Decor::of(DecorId::boat).row
}

const REGION: [[i32; 2]; 2] = [[1000, 2000], [1256, 2256]];

#[test]
fn no_two_candidates_that_stand_claim_one_cell() {
    let mut out = Vec::new();
    scatter(boat(), LEAF_ZOOM, REGION, &mut out).unwrap();
    assert!(out.len() > 50, "{}", out.len());
    for (i, a) in out.iter().enumerate() {
        for b in &out[i + 1..] {
            assert!(!meet(a, side_of(boat(), a), b, side_of(boat(), b)), "{a:?} {b:?}");
        }
    }
}

#[test]
fn a_region_keeps_on_each_cell_what_the_cell_alone_keeps() {
    let mut out = Vec::new();
    scatter(boat(), LEAF_ZOOM, REGION, &mut out).unwrap();
    for y in REGION[0][1]..REGION[1][1] {
        for x in REGION[0][0]..REGION[1][0] {
            let at = out.iter().find(|c| c.cell == [x, y]).copied();
            assert_eq!(at, scatter_at(boat(), [x, y]), "{x}, {y}");
        }
    }
}

#[test]
fn a_finer_zoom_keeps_what_a_coarser_one_had() {
    let (mut coarse, mut fine) = (Vec::new(), Vec::new());
    scatter(boat(), LEAF_ZOOM - 2, REGION, &mut coarse).unwrap();
    scatter(boat(), LEAF_ZOOM, REGION, &mut fine).unwrap();
    assert!(!coarse.is_empty());
    for c in &coarse {
        assert!(fine.contains(c), "{c:?}");
    }
}

/// Caves are some of the ground, not none and not most, over leaves spread
/// across many of the caves' first cubes rather than in a few, where one
/// cave decides the share.
#[test]
fn caves_hollow_a_little_of_the_ground() {
    let mut n = 0;
    for x in 0..64 {
        for y in 0..64 {
            for z in -32..0 {
                n += usize::from(Cave::of(GROUND.cell(), [x * 4093, y * 4099, z * 61]).hollow());
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
    for zoom in CAVE_COARSEST - 1..GROUND.cell() {
        for x in 0..6 {
            for y in 0..6 {
                for z in -6..0 {
                    let c = [x, y, z];
                    let hollow = (0..8).filter(|&k| Cave::of(zoom + 1, [0, 1, 2].map(|a| 2 * c[a] + (k >> a & 1))).hollow()).count();
                    assert_eq!(hollow > 4, Cave::of(zoom, c).hollow(), "z{zoom} {c:?}: {hollow} of 8");
                }
            }
        }
    }
}
