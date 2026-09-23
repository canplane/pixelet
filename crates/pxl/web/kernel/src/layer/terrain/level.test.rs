// pxl-kernel/layer/terrain/level.test.rs

use pxl_spec::space::indexer::FROM_L3_IDX;

use super::*;

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
fn above_the_base_a_level_holds_what_the_level_below_does_not_whole() {
    for (x, y) in [(40, 20), (41, 21), (40, 21), (-7, 3)] {
        let anchor = FROM_L3_IDX(leaf(x, y));
        assert_eq!(held(1, anchor, 1).len(), (HELD_AXIS * HELD_AXIS) as usize);
        for level in 1..5 {
            let below = ring(level - 1, anchor, HELD_AXIS);
            let rim = held(level, anchor, 0);
            // Where the anchor stands in its page decides how many of the
            // level's pages the level below covers whole: one or four.
            assert!([1, 4].contains(&(ring(level, anchor, HELD_AXIS).len() - rim.len())), "{x},{y} level {level}");
            for p in ring(level, anchor, HELD_AXIS) {
                let whole = p.children().iter().all(|c| below.contains(c));
                assert_eq!(rim.contains(&p), !whole, "{x},{y} {p:?}");
            }
        }
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
    assert_eq!(window_for(MAX_DIST, MIN_POLAR, 0.0, 16.0 / 9.0), PAGE_TOP_LEVEL..PAGE_TOP_LEVEL + 1);
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
