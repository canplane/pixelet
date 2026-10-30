// pxl-spec-core/scatter.test.rs

extern crate std;

use super::*;
use crate::window::FIRST_CELL;
use std::vec::Vec;

/// A decor's cells hold one at its chance: over a square of leaf cells, the
/// share holding a candidate is its density, within what a fair coin would
/// stray by.
#[test]
fn cells_hold_their_chance() {
    let side = 512;
    for decor in &DECORS {
        let mut held = 0u32;
        for y in 0..side {
            for x in 0..side {
                held += u32::from(candidate_at(decor, [x - 7000, y + 123_000]).1);
            }
        }
        let n = f64::from(side * side);
        let p = f64::from(decor.chance) / 4_294_967_296.0;
        let spread = 5.0 * libm::sqrt(n * p * (1.0 - p));
        assert!(
            (f64::from(held) - n * p).abs() <= spread,
            "{}: {held} of {n}, p {p}",
            decor.val
        );
    }
}

/// What stands clear never claims a brick another standing one does, and
/// one gives way only to a neighbor before it whose brick meets its own:
/// `scatter_at` over a square, every pair asked.
#[test]
fn standing_ones_never_meet() {
    for decor in &DECORS {
        let mut standing: Vec<(Candidate, i32)> = Vec::new();
        for y in 0..96 {
            for x in 0..96 {
                if let Some(c) = scatter_at(decor, [x + 4000, y - 900]) {
                    standing.push((c, side_of(decor, &c)));
                }
            }
        }
        for (i, (a, sa)) in standing.iter().enumerate() {
            for (b, sb) in &standing[i + 1..] {
                assert!(!meet(a, *sa, b, *sb), "{}: {a:?} {b:?}", decor.val);
            }
        }
    }
}

/// A variant that stands as itself at a window stands at every finer one,
/// so the coarser windows' decor is a subset of the finer's, and every one
/// stands at the leaves'.
#[test]
fn stands_finer() {
    for decor in &DECORS {
        for v in 0..decor.variants {
            assert!(stands_at(decor, v, LEAF_CELL));
            for cell in FIRST_CELL..LEAF_CELL {
                if stands_at(decor, v, cell) {
                    assert!(stands_at(decor, v, cell + 1), "{v} at z{cell}");
                }
            }
        }
    }
}

/// A variant folds exactly where it does not stand and still claims a unit
/// of the cell: none at the leaves', its share falling eightfold a zoom
/// coarser, and a tree claiming three eighths of a cell twice its side.
#[test]
fn folds_where_it_stands_not() {
    for decor in &DECORS {
        for v in 0..decor.variants {
            assert!(!folds_at(decor, v, LEAF_CELL));
            for cell in FIRST_CELL..LEAF_CELL {
                if stands_at(decor, v, cell) {
                    assert!(!folds_at(decor, v, cell), "{v} at z{cell}");
                }
                if cell + 1 < decor.cell_zoom() {
                    let (here, finer) =
                        (fold_share(decor, v, cell), fold_share(decor, v, cell + 1));
                    assert!(
                        here <= finer.div_ceil(8),
                        "{v} at z{cell}: {here} of {finer}"
                    );
                }
            }
        }
    }
    let tree = &DECORS[0];
    assert_eq!(tree.sizes[0], [1, 1, 3]);
    assert_eq!(
        fold_share(tree, 0, LEAF_CELL - 1) * FOLD_UNIT,
        3 * answer::FULL / 8
    );
    assert!(folds_at(tree, 0, LEAF_CELL - 1));
}

/// A cell's folded decor is the room of what its leaf cells hold that is
/// folded there, on a look the decor stands on; none on a look none does,
/// and none at a leaf's cell.
#[test]
fn folded_sums_its_cells() {
    let tree = &DECORS[0];
    let look = (0..256).find(|&l| tree.allows(l)).unwrap();
    let cell = LEAF_CELL - 1;
    let mut seen = false;
    for y in 0..64 {
        for x in 0..64 {
            let mut want = 0;
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let (c, here) = candidate_at(tree, [2 * x + dx, 2 * y + dy]);
                let v = variant(tree, &c);
                if here && folds_at(tree, v, cell) {
                    want += fold_share(tree, v, cell);
                }
            }
            let (share, folds) = folded(cell, [x, y], look);
            assert_eq!(share, (want * FOLD_UNIT).min(answer::FULL), "{x},{y}");
            if want > 0 {
                seen = true;
                assert_eq!(folds, tree.fold);
            }
            assert_eq!(folded(LEAF_CELL, [x, y], look), (0, 0));
        }
    }
    assert!(seen, "a tree folds into some cell");
    let none = (1..256)
        .find(|&l| DECORS.iter().all(|d| !d.allows(l)))
        .unwrap();
    assert_eq!(folded(cell, [3, 4], none), (0, 0));
}
