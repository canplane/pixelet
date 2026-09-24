// pxl-spec/space/pyramid.test.rs

use super::*;

/// The hierarchy is one algebra in any number of dimensions: a child's
/// parent and slot give it back, and a parent's children are the 2^D nodes
/// whose parent it is.
#[test]
fn a_child_is_its_parent_and_slot_in_two_and_three_dimensions() {
    for at in [[5i64, -3], [-1, -1], [0, 8]] {
        assert_eq!(child_of(parent_of(at), slot_of(at)), at);
        assert!((0..4).all(|k| parent_of(child_of(at, k)) == at && slot_of(child_of(at, k)) == k));
    }
    for at in [[5i64, -3, 2], [-1, -1, -1], [0, 8, 7]] {
        assert_eq!(child_of(parent_of(at), slot_of(at)), at);
        assert!((0..8).all(|k| parent_of(child_of(at, k)) == at && slot_of(child_of(at, k)) == k));
    }
}

/// A block of cells is a cell of a coarser level, so a cell's centre and
/// its block's are one computation: the block's cell `up` levels over, its
/// XY the middle of its span and its Z half its side above the ground.
#[test]
fn a_cells_centre_is_its_middle_and_half_its_side_above_the_ground() {
    let point = Pvf3 {
        i: ivec::gather(-1234, 5678, 0),
        ..Default::default()
    };
    for level in [0, 3, 7] {
        let cell = Cell::of(level, &point);
        assert_eq!(cell.level(), level);
        assert_eq!(Cell::of(level, &cell.base()), cell, "a cell is found from its own corner");
        assert_eq!(cell.up(2), Cell::of(level + 2, &point), "a block is the cell over it");
        let side = cell.side();
        let mut at = [0.0; 3];
        let c = cell.centre(10.0);
        projection::unproject(&mut at, c.i, c.f);
        let base = cell.base();
        assert_eq!(at[2], 10.0 + side / 2.0);
        assert_eq!(at[0], pvf3::to_real(pvf3::X(&base)) + side / 2.0);
        assert_eq!(at[1], pvf3::to_real(pvf3::Y(&base)) + side / 2.0);
    }
}

/// A lattice is shifts against the unit, so two lattices whose cells are the
/// same size cut the same ground: a lattice two shifts coarser than the
/// ground has at level 0 the ground's level-2 cell, held in a level-0 page
/// rather than a level-2 one.
#[test]
fn a_coarser_lattices_cell_is_the_grounds_cell_as_many_levels_up() {
    let coarse = Lattice::new(GROUND.cell_shamt(0) + 2);
    let point = Pvf3 {
        i: ivec::gather(-1234, 5678, 0),
        ..Default::default()
    };
    for level in [0, 3] {
        let (a, b) = (coarse.cell_of(level, &point), GROUND.cell_of(level + 2, &point));
        assert_eq!(a.side(), b.side());
        assert_eq!(a.base(), b.base());
        assert_eq!(a.page().level + 2, b.page().level);
        assert_eq!(coarse.page_cells() * 4, GROUND.page_cells());
    }
}
