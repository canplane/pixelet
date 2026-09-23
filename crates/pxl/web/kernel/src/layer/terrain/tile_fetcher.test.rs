// pxl-kernel/layer/terrain/tile_fetcher.test.rs
//
// What terrain says about a page, and where that lands. The facts are pure
// functions of where the anchor is and what the page is, so nothing here
// needs a camera.

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

/// Where a page lands, given what terrain would say about it.
fn rank(p: LevelPage, resolved: Resolved, speculative: bool) -> Rank {
    policy::terrain(resolved, speculative, centre_dist(anchor(), p) * p.span())
}

#[test]
fn the_ground_the_view_looks_at_is_asked_for_first_whatever_level_draws_it() {
    // The finest page over the anchor is the ground under the camera, and it
    // is the nearest thing there is. Ordering in a page's own spans made
    // every level's centre page equal and the screen filled everywhere at
    // once; in world units the middle comes first.
    let near = |p| rank(p, Resolved::Missing, false).distance;
    let middle = at(0, 0);
    for level in 1..WINDOW_LEVELS {
        assert!(near(middle) < near(at(level, 0)), "the leaf's centre beats level {level}'s");
        assert!(near(middle) < near(at(level, 1)), "and level {level} a page out");
    }
    // And within a level it is still the plain distance.
    assert!(near(at(2, 0)) < near(at(2, 1)));
    assert!(near(at(2, 1)) < near(at(2, 2)));
}

#[test]
fn a_class_cannot_be_crossed_by_a_distance() {
    // What nothing draws goes before what is merely coarse, wherever each of
    // them is: the class is a field of its own, so no distance reaches it.
    let every = |resolved, speculative| {
        let mut out: Vec<u8> = Vec::new();
        for level in 0..WINDOW_LEVELS {
            for dx in -(HELD_AXIS as i64 / 2)..=HELD_AXIS as i64 / 2 {
                out.push(rank(at(level, dx), resolved, speculative).class);
            }
        }
        out
    };
    let worst = |v: &[u8]| v.iter().copied().max().expect("a page");
    let best = |v: &[u8]| v.iter().copied().min().expect("a page");
    let missing = every(Resolved::Missing, false);
    let covered = every(Resolved::Covered, false);
    let rim = every(Resolved::Missing, true);
    assert!(worst(&missing) < best(&covered), "a hole before what is only coarse");
    assert!(worst(&covered) < best(&rim), "and both before the rim");
    assert!(worst(&rim) < policy::pinned().class, "the device's own page behind them all");
    assert!(policy::ocean().class < best(&missing), "and the shared tile before every hole");
}
