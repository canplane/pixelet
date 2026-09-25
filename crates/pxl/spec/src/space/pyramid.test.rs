// pxl-spec/space/pyramid.test.rs

use super::*;

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
        let cell = GROUND.cell_of(level, &point);
        assert_eq!(cell.level(), level);
        assert_eq!(GROUND.cell_of(level, &cell.base()), cell, "a cell is found from its own corner");
        assert_eq!(cell.up(2), GROUND.cell_of(level + 2, &point), "a block is the cell over it");
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

/// A fine cell found by its offset in a coarse one is the cell the offset was
/// read from, whichever lattices the two are cut in and however far apart
/// their levels are.
#[test]
fn a_cell_found_by_its_offset_in_a_coarser_one_is_itself() {
    let coarse = Lattice::new(GROUND.scale(0).up(2));
    for (x, y) in [(-1234, 5678), (0, 0), (97, -3)] {
        let point = Pvf3 {
            i: ivec::gather(x, y, 0),
            ..Default::default()
        };
        let fine = GROUND.cell_of(0, &point);
        for level in [0, 4, TOP_LEVEL] {
            let outer = coarse.cell_of(level, &point);
            let offset = outer.offset_of(fine);
            assert!(offset.iter().all(|&o| (o as f64) < outer.side() / fine.side()), "an offset inside the outer cell");
            assert_eq!(outer.inner_at(GROUND, 0, offset), fine);
        }
    }
}

/// A cell's place in a page some levels over it, counted in its own sides,
/// is its own place where the page is its own, and lies inside the cell of
/// that page it falls in: a block of them per page cell, as a chunk read from
/// a coarser page reads it.
#[test]
fn a_cells_place_in_a_page_above_lies_in_the_page_cell_over_it() {
    for (x, y) in [(-1234, 5678), (0, 0), (97, -3), (400_000, -250_000)] {
        let point = Pvf3 {
            i: ivec::gather(x, y, 0),
            ..Default::default()
        };
        for level in [0, 3] {
            let cell = GROUND.cell_of(level, &point);
            let [tx, ty] = cell.in_page();
            assert_eq!(cell.in_page_above(0), [tx as i64, ty as i64]);
            for levels in [1, 4, TOP_LEVEL - level] {
                let up = cell.up(levels);
                let n = cell.scale().count_in(up.scale()) as i64;
                let [ax, ay] = up.in_page();
                let [x, y] = cell.in_page_above(levels);
                assert_eq!([x.div_euclid(n), y.div_euclid(n)], [ax as i64, ay as i64], "level {level}, {levels} up");
            }
        }
    }
}

/// A lattice is shifts against the unit, so two lattices whose cells are the
/// same size cut the same ground: a lattice two shifts coarser than the
/// ground has at z16 the ground's z14 cell, held in a z16 page
/// rather than a level-2 one.
#[test]
fn a_coarser_lattices_cell_is_the_grounds_cell_as_many_levels_up() {
    let coarse = Lattice::new(GROUND.scale(0).up(2));
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

/// A page's children in its neighbourhood are its children, and a cell and
/// the block it folds from are parent and children whichever child page the
/// cell lies in: `child_block` gives back the cell `parent_cell` names, in
/// every lattice a page is cut into.
#[test]
fn a_parent_cell_and_its_child_block_are_one_relation() {
    let p = LevelPage::new(3, ivec::gather(-5, 7, 0));
    let around = neighbourhood(p);
    assert!((0..4).all(|q| around[CHILDREN[q]] == p.children()[q]));
    for lattice in [GROUND, Lattice::new(GROUND.scale(0).up(4))] {
        let n = lattice.page_cells();
        for q in 0..4 {
            for (tx, ty) in [(0, 0), (1, 0), (n - 1, n - 1), (n / 2, 3), (7, n - 2)] {
                let child = lattice.cell_index(tx, ty);
                let parent = lattice.parent_cell(q, child);
                let (slot, first) = lattice.child_block(parent);
                assert_eq!(slot, q, "the block lies in the child page the cell does");
                let block = [first, first + 1, first + n, first + n + 1];
                assert!(block.contains(&child), "{child} folds from the block at {first}");
            }
        }
    }
}
