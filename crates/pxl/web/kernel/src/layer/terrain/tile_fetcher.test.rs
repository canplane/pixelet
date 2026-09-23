// pxl-kernel/layer/terrain/tile_fetcher.test.rs
//
// The fetch order. `priority` is a pure function of where the anchor is and
// what the page is, so nothing here needs a camera.

use pxl_spec::space::indexer::L3_IDX;
use pxl_spec::space::pvf3::ivec;

use super::*;
use crate::layer::terrain::level::{HELD_AXIS, WINDOW_LEVELS};

fn anchor() -> IVec {
    ivec::gather(3_000_000, 2_000_000, 0)
}

/// The page `anchor` stands in at `level`, and its neighbour `dx` pages out.
fn at(level: u32, dx: i64) -> LevelPage {
    let centre = LevelPage::over(level, TO_XY(L3_IDX(anchor())));
    LevelPage::new(level, ivec::add(centre.page, ivec::gather(dx, 0, 0)))
}

fn rank(p: LevelPage, want: Want) -> f64 {
    priority(anchor(), p, want)
}

#[test]
fn the_ground_the_view_looks_at_is_asked_for_first_whatever_level_draws_it() {
    // The finest page over the anchor is the ground under the camera, and it
    // is the nearest thing there is. Ordering in a page's own spans made
    // every level's centre page equal and the screen filled everywhere at
    // once; in world units the middle comes first.
    let middle = at(0, 0);
    for level in 1..WINDOW_LEVELS {
        assert!(rank(middle, Want::Coverage) < rank(at(level, 0), Want::Coverage), "the leaf's centre beats level {level}'s");
        assert!(rank(middle, Want::Coverage) < rank(at(level, 1), Want::Coverage), "and level {level} a page out");
    }
    // And within a level it is still the plain distance.
    assert!(rank(at(2, 0), Want::Coverage) < rank(at(2, 1), Want::Coverage));
    assert!(rank(at(2, 1), Want::Coverage) < rank(at(2, 2), Want::Coverage));
}

#[test]
fn a_class_cannot_be_crossed_by_a_distance() {
    // Coverage at the far rim still beats refinement under the camera: what
    // nothing draws goes before what is merely coarse.
    let every = |want: Want| {
        let mut out: Vec<f64> = Vec::new();
        for level in 0..WINDOW_LEVELS {
            for dx in -(HELD_AXIS as i64 / 2)..=HELD_AXIS as i64 / 2 {
                out.push(rank(at(level, dx), want));
            }
        }
        out
    };
    let worst = |v: &[f64]| v.iter().copied().fold(f64::MIN, f64::max);
    let best = |v: &[f64]| v.iter().copied().fold(f64::MAX, f64::min);
    let (coverage, refinement) = (every(Want::Coverage), every(Want::Refinement));
    let prefetch = every(Want::Prefetch);
    assert!(worst(&coverage) < best(&refinement), "coverage before refinement");
    assert!(worst(&refinement) < best(&prefetch), "refinement before the rim");
    assert!(worst(&prefetch) < PINNED_PAGE_PRIORITY, "and the device's own page behind them all");
}

#[test]
fn the_distance_never_reaches_the_step_above_it() {
    // One number carries both, so what makes it lexicographic is that the
    // classes are two apart and the distance is under one — which is also
    // what leaves the odd numbers to POI.
    for level in 0..WINDOW_LEVELS {
        for dx in -(HELD_AXIS as i64)..=HELD_AXIS as i64 {
            let r = rank(at(level, dx), Want::Coverage);
            assert!(r.fract() < 1.0 && r < 2.0, "level {level} at {dx}: {r}");
        }
    }
    // A page at the far rim of the widest level is still told apart from one
    // beside it: the squash is monotone and a double has the digits.
    let far = WINDOW_LEVELS - 1;
    assert!(rank(at(far, 2), Want::Coverage) < rank(at(far, 3), Want::Coverage));
}
