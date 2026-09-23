// pxl-kernel/layer/terrain/tile_fetcher.test.rs
//
// The fetch order. `priority` is a pure function of where the anchor is and
// what the page is, so nothing here needs a camera.

use pxl_spec::space::indexer::L3_IDX;
use pxl_spec::space::pvf3::ivec;

use super::*;

/// A window five levels wide, as `terrain_demands` reads it.
const TOP: u32 = WINDOW_LEVELS - 1;

fn anchor() -> IVec {
    ivec::gather(3_000_000, 2_000_000, 0)
}

/// The page `anchor` stands in at `level`, and its neighbour `dx` rings out.
fn at(level: u32, dx: i64) -> LevelPage {
    let centre = LevelPage::over(level, TO_XY(L3_IDX(anchor())));
    LevelPage::new(level, ivec::add(centre.page, ivec::gather(dx, 0, 0)))
}

fn rank(p: LevelPage, want: Want) -> f64 {
    priority(anchor(), p, TOP, want)
}

#[test]
fn the_middle_of_the_screen_is_asked_for_at_every_level_before_the_ring_past_it() {
    // What the old order could not do: the page under the camera, at the
    // level that draws the ground right in front of it, came after the whole
    // of the coarsest ring. A page's distance is measured in its own spans,
    // so nothing under the level could have said otherwise.
    for level in 0..WINDOW_LEVELS {
        assert!(rank(at(0, 0), Want::Coverage) < rank(at(level, 1), Want::Coverage), "the centre leaf beats level {level} one ring out");
        assert!(rank(at(level, 0), Want::Coverage) < rank(at(TOP, 1), Want::Coverage), "level {level}'s centre beats the top's ring");
    }
}

#[test]
fn the_coarsest_level_of_a_ring_comes_first_within_it() {
    for ring in 0..RINGS as i64 {
        for level in 0..TOP {
            assert!(rank(at(TOP, ring), Want::Coverage) < rank(at(level, ring), Want::Coverage), "ring {ring}, level {level}");
        }
    }
}

#[test]
fn a_class_cannot_be_crossed_by_anything_under_it() {
    // Every page of a class, against every page of the next.
    let every = |want: Want| {
        let mut out: Vec<f64> = Vec::new();
        for level in 0..WINDOW_LEVELS {
            for dx in -(RINGS as i64)..=RINGS as i64 {
                out.push(rank(at(level, dx), want));
            }
        }
        out
    };
    let (coverage, refinement, prefetch) = (every(Want::Coverage), every(Want::Refinement), every(Want::Prefetch));
    let worst = |v: &[f64]| v.iter().copied().fold(f64::MIN, f64::max);
    let best = |v: &[f64]| v.iter().copied().fold(f64::MAX, f64::min);
    assert!(worst(&coverage) < best(&refinement), "coverage before refinement");
    assert!(worst(&refinement) < best(&prefetch), "refinement before prefetch");
    assert!(worst(&prefetch) < PINNED_PAGE_PRIORITY, "and the device's own page behind them all");
}

#[test]
fn the_distance_never_reaches_the_step_above_it() {
    // The terms are packed into one number, so what makes the packing
    // lexicographic is that each term's span is the one below it: the steps
    // are two apart and the distance is under one, which is also what leaves
    // the odd numbers to POI.
    for level in 0..WINDOW_LEVELS {
        for dx in -(RINGS as i64)..=RINGS as i64 {
            let r = rank(at(level, dx), Want::Coverage);
            assert!(r.fract() < 1.0 && (r - r.fract()) % 2.0 == 0.0, "level {level} ring {dx}: {r}");
        }
    }
}
