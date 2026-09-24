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
    let addr = ivec::gather(-1234, 5678, 0);
    for level in [0, 3, 7] {
        let cell = Cell::of(level, addr);
        assert_eq!(cell.level(), level);
        assert_eq!(Cell::of(level, cell.base()), cell, "a cell is found from its own corner");
        assert_eq!(cell.up(2), Cell::of(level + 2, addr), "a block is the cell over it");
        let side = cell.side();
        let mut at = [0.0; 3];
        let c = cell.centre(10.0);
        projection::unproject(&mut at, c.i, c.f);
        assert_eq!(at[2], 10.0 + side / 2.0);
        assert_eq!(at[0], ivec::X(cell.base()) as f64 + side / 2.0);
        assert_eq!(at[1], ivec::Y(cell.base()) as f64 + side / 2.0);
    }
}
