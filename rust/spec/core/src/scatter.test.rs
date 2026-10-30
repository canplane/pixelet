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

/// A column's count is its four's together, so a coarse cell's decor is
/// what its leaf cells hold, and none is past what its leaf cells can.
#[test]
fn counts_hand_down_whole() {
    for (k, root) in ROOTS.iter().enumerate() {
        for z in *root..LEAF_CELL {
            for b in [[0, 0], [5, -3], [-77, 1234], [4000, 9]] {
                let parent = count(k, z, b);
                let mut children = 0;
                for q in 0..4 {
                    children += count(k, z + 1, [2 * b[0] + (q & 1), 2 * b[1] + (q >> 1)]);
                }
                assert!(children <= parent, "{k} at z{z} {b:?}");
                assert!(parent <= 1 << (2 * (LEAF_CELL - z)), "{k} at z{z} {b:?}");
            }
        }
        for c in [[0, 0], [3, 3]] {
            assert!(count(k, LEAF_CELL, c) <= 1);
        }
    }
}

/// A cell's folded decor is its column's count of the decor that stands on
/// its look, each leaf cell filled whole and folded as the objects' tree
/// folds a placed one's (`overwrite::share`), its kind the decor's; none on
/// a look none stands on, and none at a leaf's cell.
#[test]
fn folded_counts_its_column() {
    let tree = &DECORS[0];
    let look = (0..256).find(|&l| tree.allows(l)).unwrap();
    let cell = LEAF_CELL - 1;
    let mut seen = false;
    for y in 0..64 {
        for x in 0..64 {
            let n = count(0, cell, [x, y]);
            let (share, kind) = folded(cell, [x, y], look);
            assert_eq!(share, n * answer::FULL / 8, "{x},{y}");
            if n > 0 {
                seen = true;
                assert_eq!(kind, tree.val);
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

/// A placed decor's leaf folds in the objects' tree as a scattered one's
/// does in its column: its cell filled whole, an eighth of the cell over it,
/// its kind its stored value.
#[test]
fn placed_and_scattered_fold_alike() {
    use crate::overwrite::{fold, share};
    let tree = DECORS[0].val;
    let mut children = [0u32; 8];
    children[3] = share(tree);
    let folded = fold(children, LEAF_CELL);
    assert_eq!(folded & 0xffff, tree);
    assert_eq!(folded >> 16, answer::FULL / 8);
}
