// pxl-kernel/layer/decor/build.test.rs

use super::*;

/// Coarser levels keep a subset of the finer ones' instances, so a zoom never moves one.
#[test]
fn a_levels_candidate_is_the_candidate_of_a_cell_under_it() {
    for tag in [1u64, 0xdead_beef] {
        for (x, y) in [(0i64, 0i64), (-3, 7), (12345, -678)] {
            assert_eq!(representative(tag, 0, x, y), (x, y), "a z16 cell is its own candidate");
            for level in 1..=6u32 {
                let (gx, gy) = representative(tag, level, x, y);
                let side = 1i64 << level;
                assert_eq!((gx.div_euclid(side), gy.div_euclid(side)), (x, y), "inside its cell");
                let half = side / 2;
                let child = (gx.div_euclid(half), gy.div_euclid(half));
                assert_eq!(representative(tag, level - 1, child.0, child.1), (gx, gy), "the candidate of the cell under it");
            }
        }
    }
}
