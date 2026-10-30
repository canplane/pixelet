// pxl-spec-core/scatter.test.rs

use super::*;
use crate::cast::Spacing;
use crate::ground::{Cell, VOID};

/// A patch's thinning drawn once for all its candidates (`deleted`) answers
/// each as `retained` does, over every decor of the catalog and the patches of
/// bricks at the leaves and coarser.
#[test]
fn patch_thinning() {
    let (mut compared, mut lost) = (0, 0);
    for decor in &DECORS {
        for zoom in [LEAF_ZOOM, LEAF_ZOOM - 3, TOP_ZOOM + 1] {
            let side = 1i32 << (BRICK_SHIFT + LEAF_ZOOM - zoom);
            for q in [[7, -3], [-12, 40]] {
                let region = [
                    [q[0] * side, q[1] * side],
                    [(q[0] + 1) * side, (q[1] + 1) * side],
                ];
                let mut p = 0;
                loop {
                    let (born, b, real) = patch_of(zoom, region, p);
                    if !real {
                        break;
                    }
                    let mut cs = [Candidate::NONE; COUNTS];
                    let n = each_in_patch(decor, born, b, |i, c| cs[i as usize] = c);
                    let mut asked = [0u32; MARKS];
                    for i in 0..n as usize {
                        mark(&mut asked, i);
                    }
                    let deleted = deleted(decor, zoom, &cs, n, &asked);
                    for (i, c) in cs.iter().enumerate().take(n as usize) {
                        assert_eq!(
                            !is_marked(&deleted, i),
                            retained(decor, zoom, c),
                            "decor {} at {zoom}, patch {p}, candidate {i}",
                            decor.val
                        );
                        compared += 1;
                        lost += usize::from(is_marked(&deleted, i));
                    }
                    p += 1;
                }
            }
        }
    }
    assert!(compared > 1000, "only {compared} candidates compared");
    assert!(
        lost > 0,
        "no candidate was deleted, so the thinning was not tried"
    );
}

/// Decor of the leaves' window, or placed, is drawn where the rays ask the
/// leaf cell's zoom or finer at its distance; decor of a coarser window
/// where they ask that window's zoom alone, so a distance draws one window's.
#[test]
fn decor_at_its_window() {
    let detail = Detail {
        range: [0, crate::FINEST_ZOOM],
        eye: [0.0; 3],
        spacing: Spacing::NONE,
    };
    let mut d = 0.25f32;
    let mut seen = [false; 2];
    while d < 1.0e6 {
        let d2 = d * d;
        let at = detail.at(d2);
        for cell in [LEAF_CELL, PLACED_AT] {
            assert_eq!(drawn_at(detail, d2, cell), at >= LEAF_CELL, "at {d}: z{at}");
        }
        for cell in window::FIRST_CELL..LEAF_CELL {
            assert_eq!(
                drawn_at(detail, d2, cell),
                at == cell,
                "at {d}: z{at}, z{cell}"
            );
        }
        seen[usize::from(at >= LEAF_CELL)] = true;
        d *= 1.5;
    }
    assert_eq!(seen, [true, true], "both sides of the leaves are asked");
}

/// A variant that stands as itself at a window stands at every finer one,
/// so the coarser windows' decor is a subset of the finer's, and every one
/// stands at the leaves'.
#[test]
fn stands_finer() {
    for decor in &DECORS {
        for v in 0..decor.variants {
            assert!(stands_at(decor, v, LEAF_CELL));
            for cell in window::FIRST_CELL..LEAF_CELL {
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
            for cell in window::FIRST_CELL..LEAF_CELL {
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
        fold_share(tree, 0, LEAF_CELL - 1) * window::FOLD_UNIT,
        3 * answer::FULL / 8
    );
    assert!(folds_at(tree, 0, LEAF_CELL - 1));
}

/// A column word keeps its ground's top under what is folded over it, its
/// share held to a byte and its look the greatest folded, whatever the order;
/// and a column's folded decor lies in its top cell alone, in the look its
/// decors name or the ground's.
#[test]
fn folded_over_the_top() {
    let tree = &DECORS[0];
    let share = fold_share(tree, 0, LEAF_CELL - 1);
    let top = window::column_word(-37);
    let w = window::fold_into(window::fold_into(top, share, 0), share, tree.fold);
    assert_eq!(
        w,
        window::fold_into(window::fold_into(top, share, tree.fold), share, 0)
    );
    assert_eq!(window::column_top(w), -37);
    assert_eq!(
        window::column_fold(w),
        (2 * share * window::FOLD_UNIT, tree.fold)
    );
    let full = window::fold_into(window::column_word(5), 200, 0);
    assert_eq!(window::column_top(window::fold_into(full, 200, 3)), 5);
    assert_eq!(
        window::column_fold(window::fold_into(full, 200, 3)),
        (answer::FULL, 3)
    );
    let cell = Cell {
        look: 7,
        top: 3,
        part: answer::FULL / 2,
        fold: [7; 4],
        decor: answer::FULL / 4,
        decor_look: 0,
    };
    assert_eq!(cell.folded(2), (answer::FULL / 4, 7));
    assert_eq!(cell.folded(1), (0, VOID));
    assert_eq!(cell.folded(3), (0, VOID));
    let named = Cell {
        decor_look: tree.fold,
        ..cell
    };
    assert_eq!(named.folded(2), (answer::FULL / 4, tree.fold));
}

/// A window's brick at any zoom meets no more patches than `BRICK_PATCHES`,
/// the bound its records are laid out by, worked out at the leaves'.
#[test]
fn brick_patches_any_window() {
    for cell in window::FIRST_CELL..=LEAF_CELL {
        let zoom = GROUND.read_zoom(cell);
        for q in [[0, 0], [7, -3], [-12, 40]] {
            let [x0, y0, x1, y1] = brick_box(cell, q);
            let mut n = 0;
            while patch_of(zoom, [[x0, y0], [x1, y1]], n).2 {
                n += 1;
            }
            assert!(n <= BRICK_PATCHES, "z{cell} {q:?}: {n} > {BRICK_PATCHES}");
        }
    }
}
