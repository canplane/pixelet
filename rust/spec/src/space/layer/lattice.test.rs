// pxl-spec/space/layer/lattice.test.rs

use super::*;
use crate::space::coord::zoom::UNIT_ZOOM;
use crate::space::layer::quadrant::{CHILDREN, neighborhood};

/// A cell is found from its own corner, and the cell `up` zooms coarser is the point's cell there.
#[test]
fn cell_from_point() {
    let point = base_of(UNIT_ZOOM, [-1234, 5678, 0]);
    for zoom in [LEAF_ZOOM, LEAF_ZOOM - 3, LEAF_ZOOM - 7] {
        let cell = GROUND.cell_of(zoom, &point);
        assert_eq!(cell.tile_zoom(), zoom);
        assert_eq!(
            GROUND.cell_of(zoom, &cell.base()),
            cell,
            "a cell is found from its own corner"
        );
        assert_eq!(
            cell.up(2),
            GROUND.cell_of(zoom - 2, &point),
            "the cell over it"
        );
    }
}

/// Two lattices with equal cell sizes cut the same ground.
#[test]
fn lattice_cell_off() {
    let coarse = Lattice::new(GROUND.cell_zoom(LEAF_ZOOM) - 2);
    let point = base_of(UNIT_ZOOM, [-1234, 5678, 0]);
    for zoom in [LEAF_ZOOM, 13] {
        let (a, b) = (
            coarse.cell_of(zoom, &point),
            GROUND.cell_of(zoom - 2, &point),
        );
        assert_eq!(a.side(), b.side());
        assert_eq!(a.base(), b.base());
        assert_eq!(a.tile().zoom, b.tile().zoom + 2);
        assert_eq!(coarse.tile_cells() * 4, GROUND.tile_cells());
    }
}

/// `child_cells` inverts `parent_cell` in every lattice.
#[test]
fn parent_child_cells() {
    let p = Quadrant::new(13, [-5, 7]);
    let around = neighborhood(p);
    assert!((0..4).all(|q| around[CHILDREN[q]] == p.children()[q]));
    for lattice in [GROUND, Lattice::new(GROUND.cell_zoom(LEAF_ZOOM) - 4)] {
        let n = lattice.tile_cells();
        for q in 0..4 {
            for (tx, ty) in [(0, 0), (1, 0), (n - 1, n - 1), (n / 2, 3), (7, n - 2)] {
                let child = lattice.cell_idx(tx, ty);
                let parent = lattice.parent_cell(q, child);
                let (slot, cells) = lattice.child_cells(parent);
                assert_eq!(slot, q, "the cells lie in the child tile the cell does");
                assert_eq!(
                    cells,
                    [cells[0], cells[0] + 1, cells[0] + n, cells[0] + n + 1],
                    "2×2 cells, row by row"
                );
                assert!(
                    cells.contains(&child),
                    "{child} folds from the cells at {}",
                    cells[0]
                );
            }
        }
    }
}
