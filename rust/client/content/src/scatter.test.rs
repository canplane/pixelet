// pxl-content/scatter.test.rs

use super::*;
use pxl_engine::clipmap::FIRST_CELL_ZOOM;
use pxl_spec::share::{eighths, FULL};

/// A decor's cells hold one at its chance: over a square of leaf cells, the
/// share holding a candidate is its density, within what a fair coin would
/// stray by.
#[test]
fn cells_hold_their_chance() {
    let side = 512;
    for (k, decor) in ROWS.iter().enumerate() {
        let mut cnt = 0u32;
        for y in 0..side {
            for x in 0..side {
                cnt += u32::from(candidate_at(k, [x - 7000, y + 123_000]).1);
            }
        }
        let n = f64::from(side * side);
        let p = f64::from(decor.chance) / 4_294_967_296.0;
        let spread = 5.0 * libm::sqrt(n * p * (1.0 - p));
        assert!(
            (f64::from(cnt) - n * p).abs() <= spread,
            "decor {k}: {cnt} of {n}, p {p}"
        );
    }
}

/// What stands clear never claims a brick another standing one does, and
/// one gives way only to a neighbor before it whose brick meets its own:
/// `scatter_at` over a square, every pair asked.
#[test]
fn standing_ones_never_meet() {
    for (k, decor) in ROWS.iter().enumerate() {
        let mut standing: Vec<(Candidate, i32)> = Vec::new();
        for y in 0..96 {
            for x in 0..96 {
                if let Some(c) = scatter_at(k, [x + 4000, y - 900]) {
                    standing.push((c, side_of(decor, &c)));
                }
            }
        }
        for (i, (a, sa)) in standing.iter().enumerate() {
            for (b, sb) in &standing[i + 1..] {
                assert!(!meet(a, *sa, b, *sb), "decor {k}: {a:?} {b:?}");
            }
        }
    }
}

/// A variant that stands as itself at a clip level stands at every finer one,
/// so the coarser clip levels' decor is a subset of the finer's, and every one
/// stands at the leaves'.
#[test]
fn stands_finer() {
    for decor in &ROWS {
        for v in 0..decor.variants {
            assert!(stands_at(decor, v, LEAF_CELL_ZOOM));
            for cell_zoom in FIRST_CELL_ZOOM..LEAF_CELL_ZOOM {
                if stands_at(decor, v, cell_zoom) {
                    assert!(stands_at(decor, v, cell_zoom + 1), "{v} at z{cell_zoom}");
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
        for z in *root..LEAF_CELL_ZOOM {
            for b in [[0, 0], [5, -3], [-77, 1234], [4000, 9]] {
                let parent = cnt(k, z, b);
                let mut children = 0;
                for q in 0..4 {
                    children += cnt(k, z + 1, [2 * b[0] + (q & 1), 2 * b[1] + (q >> 1)]);
                }
                assert!(children <= parent, "{k} at z{z} {b:?}");
                assert!(
                    parent <= 1 << (2 * (LEAF_CELL_ZOOM - z)),
                    "{k} at z{z} {b:?}"
                );
            }
        }
        for c in [[0, 0], [3, 3]] {
            assert!(cnt(k, LEAF_CELL_ZOOM, c) <= 1);
        }
    }
}

/// A decor stands where its box keeps an eighth of the clip level's cell, as a folded cube of
/// that box is drawn (`share::eighths`). This holds for every variant of every decor; a tree
/// a unit wide and three high stands one zoom coarser than the leaves', and not two.
#[test]
fn stands_as_a_cube_keeps() {
    for d in &ROWS {
        for v in 0..d.variants {
            let [x, y, z] = d.sizes[v as usize];
            for cell_zoom in FIRST_CELL_ZOOM..=LEAF_CELL_ZOOM {
                let steps = 3 * (UNIT_ZOOM - cell_zoom.min(UNIT_ZOOM));
                let fill = (u64::from(x * y * z) * u64::from(FULL)) >> steps;
                assert_eq!(
                    stands_at(d, v, cell_zoom),
                    cell_zoom >= LEAF_CELL_ZOOM || eighths(fill.min(u64::from(FULL)) as u32) >= 1,
                    "{v} z{cell_zoom}"
                );
            }
        }
    }
    let tree = &ROWS[0];
    assert_eq!(tree.sizes[0], [1, 1, 3]);
    assert!(stands_at(tree, 0, LEAF_CELL_ZOOM - 1) && !stands_at(tree, 0, LEAF_CELL_ZOOM - 2));
}

/// A leaf cell's count comes up as often at each of its four places under
/// its parent: a lone count goes either way.
#[test]
fn counts_land_on_every_quarter() {
    let mut by = [0u32; 4];
    for k in 0..ROWS.len() {
        for y in 0..256 {
            for x in 0..256 {
                if cnt(k, LEAF_CELL_ZOOM, [x, y]) > 0 {
                    by[(x & 1 | (y & 1) << 1) as usize] += 1;
                }
            }
        }
    }
    let all: u32 = by.iter().sum();
    for n in by {
        assert!(n * 8 > all, "{by:?}");
    }
}

/// A brick at a decor's root holds no more than the room laid for it.
#[test]
fn a_brick_holds_at_most_its_room() {
    for (k, decor) in ROWS.iter().enumerate() {
        for t in -5000..5000 {
            assert!(cnt(k, ROOTS[k], [t * 7, t * 13]) <= brick_cap(decor));
        }
    }
}
