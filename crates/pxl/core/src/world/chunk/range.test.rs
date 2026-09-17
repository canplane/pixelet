// pxl_core/world/chunk/range.test.rs

use std::collections::HashSet;

use pxl_shared::spec::space::indexer::L3_IDX;
use pxl_shared::spec::space::pvf3::ivec;
use pxl_shared::spec::space::xy::TO_XY;

use super::*;
use crate::layer::environment::config::render_cfg;

fn at(x: i64, y: i64) -> IVec {
    ivec::gather(x, y, 0)
}

const RADIUS: f64 = 1536.0;

#[test]
fn range_anchor_of_snaps_to_the_chunk_not_to_the_page() {
    assert_eq!(range_anchor_of(at(1100, 70)), at(1088, 64));
    assert_eq!(range_anchor_of(at(-1, -1)), at(-64, -64));
}

#[test]
fn pages_in_radius_holds_every_page_within_the_radius_and_nothing_outside_it() {
    for (x, y) in [(0, 0), (512, 512), (1023, 1000), (1024, 0), (-1, -1), (-1025, 700)] {
        let anchor = range_anchor_of(at(x, y));
        let got = pages_in_radius(anchor, RADIUS);
        for p in &got {
            assert!(page_dist2(anchor, p.page_idx) <= RADIUS * RADIUS);
        }
        // And nothing in range is missing, over a box wider than the
        // enumeration's own.
        let have: HashSet<IVec> = got.iter().map(|p| p.page_idx).collect();
        let center_page = TO_XY(L3_IDX(anchor));
        for dy in -4..=4 {
            for dx in -4..=4 {
                let page_idx = ivec::add(center_page, at(dx, dy));
                if page_dist2(anchor, page_idx) <= RADIUS * RADIUS {
                    assert!(have.contains(&page_idx), "({x},{y}) misses page ({dx},{dy})");
                }
            }
        }
    }
}

#[test]
fn pages_in_radius_nearest_first_and_the_anchor_s_own_page_is_first() {
    let got = pages_in_radius(range_anchor_of(at(512, 512)), RADIUS);
    assert_eq!(got[0].dist2, 0.0);
    let d: Vec<f64> = got.iter().map(|p| p.dist2).collect();
    let mut sorted = d.clone();
    sorted.sort_by(f64::total_cmp);
    assert_eq!(d, sorted);
}

#[test]
fn pages_in_radius_crossing_a_page_boundary_never_drops_a_page_still_in_range() {
    let mut previous: Option<HashSet<IVec>> = None;
    let mut x = 1024 - 256;
    while x <= 1024 + 256 {
        let anchor = range_anchor_of(at(x, 0));
        let set: HashSet<IVec> = pages_in_radius(anchor, RADIUS).into_iter().map(|p| p.page_idx).collect();
        if let Some(prev) = &previous {
            for &page_idx in prev {
                if page_dist2(anchor, page_idx) <= RADIUS * RADIUS {
                    assert!(set.contains(&page_idx));
                }
            }
        }
        previous = Some(set);
        x += 64;
    }
}

#[test]
fn pages_in_radius_the_configured_range_stays_inside_its_measured_page_budget() {
    // 21 pages worst case over every anchor position, ~85MB decoded.
    let mut worst = 0;
    for y in (0..1024).step_by(64) {
        for x in (0..1024).step_by(64) {
            worst = worst.max(pages_in_radius(range_anchor_of(at(x, y)), render_cfg::CHUNK_RENDER_DIST).len());
        }
    }
    assert!(worst <= 21, "worst {worst}");
}

#[test]
fn page_priority_counts_whole_pages_of_distance_doubled_so_a_second_subsystem_can_interleave() {
    assert_eq!(page_priority(0.0), 0.0);
    assert_eq!(page_priority(1024.0 * 1024.0), 2.0);
}

#[test]
fn pages_to_hold_always_offers_the_full_count_wherever_the_anchor_stands() {
    for y in (0..1024).step_by(64) {
        for x in (0..1024).step_by(64) {
            assert_eq!(pages_to_hold(range_anchor_of(at(x, y)), 3072.0, 32).len(), 32);
        }
    }
}

#[test]
fn pages_to_hold_a_page_being_fetched_is_one_worth_keeping() {
    let anchor = range_anchor_of(at(512, 512));
    let held = pages_to_hold(anchor, 3072.0, 32);
    let keys: HashSet<IVec> = held.iter().map(|p| p.page_idx).collect();
    assert_eq!(keys.len(), held.len());
    for p in &held {
        assert_eq!(page_dist2(anchor, p.page_idx), p.dist2);
    }
}
