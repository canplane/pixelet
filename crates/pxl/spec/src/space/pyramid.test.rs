// pxl-spec/space/pyramid.test.rs

use super::*;

/// A cell's center and the coarser cell's over it are one computation: the cell `up` zooms coarser.
#[test]
fn a_cells_center_is_its_middle_and_half_its_side_above_the_ground() {
    let point = Pvf3 {
        i: ivec::gather(-1234, 5678, 0),
        ..Default::default()
    };
    for z in [LEAF_Z, LEAF_Z - 3, LEAF_Z - 7] {
        let cell = GROUND.cell_of(z, &point);
        assert_eq!(cell.z(), z);
        assert_eq!(GROUND.cell_of(z, &cell.base()), cell, "a cell is found from its own corner");
        assert_eq!(cell.up(2), GROUND.cell_of(z - 2, &point), "the cell over it");
        let side = cell.side();
        let c = cell.center(10.0);
        let at = projection::unproject(c.i, c.f);
        let base = cell.base();
        assert_eq!(at[2], 10.0 + side / 2.0);
        assert_eq!(at[0], pvf3::to_real(pvf3::X(&base)) + side / 2.0);
        assert_eq!(at[1], pvf3::to_real(pvf3::Y(&base)) + side / 2.0);
    }
}

/// A cell found by its offset in a coarser one is the cell the offset came from, across lattices and zooms.
#[test]
fn a_cell_found_by_its_offset_in_a_coarser_one_is_itself() {
    let coarse = Lattice::new(GROUND.cell_zoom(LEAF_Z) - 2);
    for (x, y) in [(-1234, 5678), (0, 0), (97, -3)] {
        let point = Pvf3 {
            i: ivec::gather(x, y, 0),
            ..Default::default()
        };
        let fine = GROUND.cell_of(LEAF_Z, &point);
        for z in [LEAF_Z, 12, TOP_Z] {
            let outer = coarse.cell_of(z, &point);
            let offset = outer.offset_of(fine);
            assert!(offset.iter().all(|&o| (o as f64) < outer.side() / fine.side()), "an offset inside the outer cell");
            assert_eq!(outer.inner_at(GROUND, LEAF_Z, offset), fine);
        }
    }
}

/// A cell's place in a tile zooms coarser, in its own sides, lies inside the tile cell it falls in.
#[test]
fn a_cells_place_in_a_tile_above_lies_in_the_tile_cell_over_it() {
    for (x, y) in [(-1234, 5678), (0, 0), (97, -3), (400_000, -250_000)] {
        let point = Pvf3 {
            i: ivec::gather(x, y, 0),
            ..Default::default()
        };
        for z in [LEAF_Z, 13] {
            let cell = GROUND.cell_of(z, &point);
            let [tx, ty] = cell.in_tile();
            assert_eq!(cell.in_tile_above(0), [tx as i64, ty as i64]);
            for steps in [1, 4, z - TOP_Z] {
                let up = cell.up(steps);
                let n = crate::space::zoom::count(cell.zoom(), up.zoom()) as i64;
                let [ax, ay] = up.in_tile();
                let [x, y] = cell.in_tile_above(steps);
                assert_eq!([x.div_euclid(n), y.div_euclid(n)], [ax as i64, ay as i64], "z{z}, {steps} up");
            }
        }
    }
}

/// Two lattices with equal cell sizes cut the same ground.
#[test]
fn a_coarser_lattices_cell_is_the_grounds_cell_as_many_zooms_coarser() {
    let coarse = Lattice::new(GROUND.cell_zoom(LEAF_Z) - 2);
    let point = Pvf3 {
        i: ivec::gather(-1234, 5678, 0),
        ..Default::default()
    };
    for z in [LEAF_Z, 13] {
        let (a, b) = (coarse.cell_of(z, &point), GROUND.cell_of(z - 2, &point));
        assert_eq!(a.side(), b.side());
        assert_eq!(a.base(), b.base());
        assert_eq!(a.tile().z, b.tile().z + 2);
        assert_eq!(coarse.tile_cells() * 4, GROUND.tile_cells());
    }
}

/// `child_cells` inverts `parent_cell` in every lattice.
#[test]
fn a_parent_cell_and_its_child_cells_are_one_relation() {
    let p = Tile::new(13, ivec::gather(-5, 7, 0));
    let around = neighborhood(p);
    assert!((0..4).all(|q| around[CHILDREN[q]] == p.children()[q]));
    for lattice in [GROUND, Lattice::new(GROUND.cell_zoom(LEAF_Z) - 4)] {
        let n = lattice.tile_cells();
        for q in 0..4 {
            for (tx, ty) in [(0, 0), (1, 0), (n - 1, n - 1), (n / 2, 3), (7, n - 2)] {
                let child = lattice.cell_index(tx, ty);
                let parent = lattice.parent_cell(q, child);
                let (slot, cells) = lattice.child_cells(parent);
                assert_eq!(slot, q, "the cells lie in the child tile the cell does");
                assert_eq!(cells, [cells[0], cells[0] + 1, cells[0] + n, cells[0] + n + 1], "2×2 cells, row by row");
                assert!(cells.contains(&child), "{child} folds from the cells at {}", cells[0]);
            }
        }
    }
}

/// X goes round the world and Y stops at the Mercator square's edge, and a
/// stored X stands for an eye at its nearest copy.
#[test]
fn a_point_goes_round_in_x_and_stops_at_the_edge_in_y() {
    use crate::space::pvf3::{nearest, projection, REACH, ROUND};
    let half = ROUND / 2.0;
    assert_eq!(projection::project(half + 3.0, REACH.max[1] + 100.0, 0.0), projection::project(-half + 3.0, REACH.max[1], 0.0));
    assert_eq!(nearest(-half + 3.0, half - 1.0), half + 3.0);
    assert_eq!(nearest(half - 1.0, -half + 3.0), -half - 1.0);
    for z in [TOP_Z, LEAF_Z] {
        let n = around(z);
        assert_eq!(tile_at(z, [n / 2, 0]), tile_at(z, [-n / 2, 0]));
        assert!(!tile_rows(z).contains(&(n / 2)));
    }
}
