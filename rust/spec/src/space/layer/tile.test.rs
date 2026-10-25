// pxl-spec/space/layer/tile.test.rs

use super::*;
use crate::space::coord::world::{nearest, ROUND};
use crate::space::coord::zoom::UNIT_ZOOM;
use crate::space::layer::lattice::Cells;
use crate::space::layer::lattice::GROUND;
use crate::space::layer::lattice::*;

/// A cell is found from its own corner, and the cell `up` zooms coarser is the point's cell there.
#[test]
fn a_cell_is_found_from_its_corner_and_the_cell_over_it_from_the_point() {
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
fn a_coarser_lattices_cell_is_the_grounds_cell_as_many_zooms_coarser() {
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
fn a_parent_cell_and_its_child_cells_are_one_relation() {
    let p = Tile::new(13, [-5, 7]);
    let around = neighborhood(p);
    assert!((0..4).all(|q| around[CHILDREN[q]] == p.children()[q]));
    for lattice in [GROUND, Lattice::new(GROUND.cell_zoom(LEAF_ZOOM) - 4)] {
        let n = lattice.tile_cells();
        for q in 0..4 {
            for (tx, ty) in [(0, 0), (1, 0), (n - 1, n - 1), (n / 2, 3), (7, n - 2)] {
                let child = lattice.cell_index(tx, ty);
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

/// X goes round the world and Y stops at the Mercator square's edge, and a
/// stored X stands for an eye at its nearest copy.
#[test]
fn a_point_goes_round_in_x_and_stops_at_the_edge_in_y() {
    use crate::space::coord::world::{self, REACH};
    let half = ROUND / 2.0;
    assert_eq!(
        world::point(half + 3.0, REACH.max[1] + 100.0, 0.0),
        world::point(-half + 3.0, REACH.max[1], 0.0)
    );
    assert_eq!(nearest(-half + 3.0, half - 1.0), half + 3.0);
    assert_eq!(nearest(half - 1.0, -half + 3.0), -half - 1.0);
    for zoom in [TOP_ZOOM, LEAF_ZOOM] {
        let n = 1i32 << zoom;
        assert_eq!(tile_at(zoom, [n / 2, 0]), tile_at(zoom, [-n / 2, 0]));
        assert!(!tile_rows(zoom).contains(&(n / 2)));
    }
}

/// A Bundang tile, (11569, 3671): rows count from the south and a first
/// digit is the signs, written out because the orientation is what can be
/// silently wrong. The bucket's keys are these names.
const BUNDANG: [i32; 2] = [11569, 3671];
const BUNDANG_NAME: &str = "010332102130223";

#[test]
fn the_known_tile_is_named_as_the_bucket_holds_it() {
    let leaf = Tile::new(LEAF_ZOOM, BUNDANG);
    assert_eq!(leaf.name(), BUNDANG_NAME);
    assert_eq!(Tile::of_name(BUNDANG_NAME), Some(leaf));
}

/// One prefix is one region at every zoom, and a name's length is its zoom.
#[test]
fn a_coarser_tile_is_named_by_a_prefix_of_the_leaf_under_it() {
    for zoom in TOP_ZOOM..LEAF_ZOOM {
        let coarse = Tile::over(zoom, BUNDANG).name();
        assert_eq!(coarse.len(), zoom as usize);
        assert!(BUNDANG_NAME.starts_with(&coarse), "z{zoom} {coarse}");
    }
}

#[test]
fn every_tile_survives_its_name_at_every_zoom() {
    let n = 1i32 << LEAF_ZOOM;
    for x in [-n / 2, -12345, -1, 0, 1, 11569, n / 2 - 1] {
        for y in [-n / 2, -7777, -1, 0, 1, 3671, n / 2 - 1] {
            for zoom in TOP_ZOOM..=LEAF_ZOOM {
                let p = Tile::over(zoom, [x, y]);
                assert_eq!(Tile::of_name(&p.name()), Some(p), "({x}, {y}) at z{zoom}");
            }
        }
    }
}

#[test]
fn what_is_not_a_held_tile_s_name_is_refused() {
    assert_eq!(Tile::of_name(""), None);
    assert_eq!(Tile::of_name("4"), None, "a digit is a zoom's two bits");
    assert_eq!(Tile::of_name("13211032031200a"), None);
    assert_eq!(
        Tile::of_name("-1321103203120"),
        None,
        "the digits carry the sign"
    );
    assert_eq!(
        Tile::of_name(&"0".repeat(LEAF_ZOOM as usize + 1)),
        None,
        "nothing is finer than a leaf"
    );
    assert_eq!(Tile::of_name("0"), None, "coarser than the top");
}
