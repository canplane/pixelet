// pxl-kernel/layer/decor/build.test.rs

use super::*;

#[test]
fn a_draw_rounds_the_word_to_a_double_first_as_javascript_does() {
    // A word past 2^53 whose low bits differ from its double's.
    let z: u64 = 0xffff_ffff_ffff_fdff;
    assert_eq!(((z as f64) % 4_294_967_296.0) as u32, 0, "rounds up to 2^64, which is 0 mod 2^32");
    assert_ne!(z as u32, 0);
}

/// A level cell's candidate is a z16 cell inside it, and the candidate of
/// the cell under it that it lies in: coarser levels keep a subset of the
/// finer ones' instances, so a zoom never moves one.
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
