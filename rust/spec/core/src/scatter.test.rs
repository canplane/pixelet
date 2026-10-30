// pxl-spec-core/scatter.test.rs

use super::*;
use crate::cast::Spacing;

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
