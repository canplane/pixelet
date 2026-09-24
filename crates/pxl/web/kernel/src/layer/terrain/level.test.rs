// pxl-kernel/layer/terrain/level.test.rs

use pxl_spec::space::indexer::FROM_L3_IDX;

use super::*;
use pxl_spec::space::pyramid::neighbourhood;

fn leaf(x: i64, y: i64) -> IVec {
    ivec::gather(x, y, 0)
}

#[test]
fn a_page_over_a_leaf_is_the_leaf_shifted_right_a_floor_on_negatives() {
    assert_eq!(LevelPage::over(2, leaf(9, 4)), LevelPage::new(2, leaf(2, 1)));
    assert_eq!(LevelPage::over(1, leaf(-1, -3)), LevelPage::new(1, leaf(-1, -2)));
    assert_eq!(LevelPage::over(0, leaf(5, 6)).parent(), LevelPage::new(1, leaf(2, 3)));
    assert_eq!(LevelPage::new(3, leaf(-2, 5)).first_leaf(), leaf(-16, 40));
}

#[test]
fn a_subject_carries_the_level_and_gives_it_back() {
    for p in [LevelPage::new(0, leaf(11569, 3671)), LevelPage::new(7, leaf(-3, 90)), LevelPage::new(10, leaf(11, -1))] {
        assert_eq!(LevelPage::of_subject(p.subject()), p);
    }
    // A leaf's subject is its key: level 0 is the lane left at zero.
    assert_eq!(LevelPage::new(0, leaf(11569, 3671)).subject(), leaf(11569, 3671));
}

#[test]
fn a_ring_is_the_axis_squared_centred_on_the_anchors_page_nearest_first() {
    let anchor = FROM_L3_IDX(leaf(40, 20));
    for level in 0..4 {
        let held = ring(level, anchor, HELD_AXIS);
        assert_eq!(held.len(), (HELD_AXIS * HELD_AXIS) as usize);
        assert_eq!(held[0], LevelPage::over(level, leaf(40, 20)));
        assert!(held.windows(2).all(|w| ring_of(anchor, w[0]) <= ring_of(anchor, w[1])));
        assert!(held.iter().all(|&p| ring_of(anchor, p) <= HELD_AXIS / 2));
        let drawn = ring(level, anchor, RENDER_AXIS);
        assert!(drawn.iter().all(|p| held.contains(p)));
    }
}

#[test]
fn a_levels_span_doubles_with_the_level() {
    assert_eq!(span_of(0), PAGE_SPAN);
    assert_eq!(span_of(4), 16.0 * PAGE_SPAN);
}

#[test]
fn a_window_holds_the_levels_the_ground_in_view_wants_and_no_others() {
    use crate::control::camera::config::camera_cfg::view::{DEFAULT_DIST, DEFAULT_POLAR, MAX_DIST, MAX_POLAR, MIN_POLAR};
    // The default view sees ground from about 10u to 92u: step 1 all of it.
    assert_eq!(window_for(DEFAULT_DIST, DEFAULT_POLAR, 0.0, 16.0 / 9.0), 0..1);
    // Looking straight down from 1,024u, the ground is 1,024u to about
    // 1,130u away: one level, the one whose cells averaged 2×2 are the step
    // that wants — step 8, from level 2.
    assert_eq!(window_for(1024.0, MIN_POLAR, 0.0, 16.0 / 9.0), 2..3);
    // From as far as the camera goes, the pyramid's top alone.
    assert_eq!(window_for(MAX_DIST, MIN_POLAR, 0.0, 16.0 / 9.0), TOP_LEVEL..TOP_LEVEL + 1);
    // Out to the horizon the fog ends it, and the window spans the octaves
    // between, never more than it is sized for.
    let w = window_for(MAX_DIST, MAX_POLAR, 0.0, 16.0 / 9.0);
    assert!(w.start < w.end && w.end - w.start <= WINDOW_LEVELS, "{w:?}");
}

#[test]
fn the_neighbourhood_is_the_one_the_bake_folds_from() {
    // The bake's own expectations (pxl-geo-bake terrain/pyramid.test.rs):
    // the same pages in the same places, so a page folded here is the page
    // the server sends.
    let around = neighbourhood(LevelPage::new(1, leaf(5, 7)));
    assert_eq!([around[5], around[6], around[9], around[10]].map(|p| p.page), [leaf(10, 14), leaf(11, 14), leaf(10, 15), leaf(11, 15)]);
    assert_eq!(around[0].page, leaf(9, 13));
    assert_eq!(around[15].page, leaf(12, 16));
    assert!(around.iter().all(|p| p.level == 0));
    // X wraps across the seam: west of the westmost is the eastmost.
    let across = 1i64 << (pxl_spec::space::pvf3::core::IVEC_X_W - pxl_spec::space::indexer::L3_SHAMT);
    let west = neighbourhood(LevelPage::new(1, leaf(-across / 4, 0)));
    assert_eq!(ivec::X(west[5].page), -across / 2);
    assert_eq!(ivec::X(west[4].page), across / 2 - 1);
}

/// What the window's top level cannot get from what is already held, and so
/// the one place a zoom out has to wait on the network before there is any
/// ground at all.
///
/// A page is folded from the four under it, so folding a level's drawn ring —
/// RENDER_AXIS pages across — wants 2·RENDER_AXIS consecutive pages of the
/// level below. The held ring is HELD_AXIS across, and five is short of six:
/// the centre folds and one neighbour per axis folds, four of nine.
///
/// Nothing stands in for the other five. The level below draws its own
/// RENDER_AXIS ring, half as wide as the ring above it, so the outer half of
/// a new top ring is ground nothing has ever drawn — and the top is the one
/// level with no ancestor inside the window, so there is nothing coarser to
/// fall back to either. Every reference clipmap keeps its coarsest level
/// resident for exactly this reason; here the coarsest level moves with the
/// camera, so it is the newest thing rather than the oldest.
///
/// Closing it costs records: HELD_AXIS 7 folds the whole ring, or a level
/// held above the window's top arrives before the window reaches it.
#[test]
fn the_window_s_top_ring_is_half_a_ring_short_of_folding() {
    use std::collections::BTreeSet;
    const _: () = assert!(2 * RENDER_AXIS > HELD_AXIS, "a ring folds from twice its own width below");
    for ax in 0..16i64 {
        for ay in 0..16i64 {
            // Swept across a page of the level below, so both parities of its
            // centre page are covered.
            let anchor = ivec::gather(3_000_000 + ax * 149, 2_000_000 + ay * 149, 0);
            let held_below: BTreeSet<LevelPage> = held(3, anchor).into_iter().collect();
            let drawn_below: BTreeSet<LevelPage> = ring(3, anchor, RENDER_AXIS).into_iter().collect();
            let drawn = ring(4, anchor, RENDER_AXIS);
            let folds = |p: &LevelPage| p.children().iter().all(|c| held_below.contains(c));
            assert_eq!(drawn.iter().filter(|p| folds(p)).count(), 4, "of {}", drawn.len());
            for p in drawn.iter().filter(|p| !folds(p)) {
                let drawn_kids = p.children().iter().filter(|c| drawn_below.contains(c)).count();
                assert_eq!(drawn_kids, 0, "nothing under {p:?} is drawn either");
            }
        }
    }
}
