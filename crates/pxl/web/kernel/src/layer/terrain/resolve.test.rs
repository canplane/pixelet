// pxl-kernel/layer/terrain/resolve.test.rs
//
// What the resident pages answer for one that is not. The store is global,
// so each test empties the levels it touches first.

use pxl_spec::bake::format::{Ground, Terrain, Tile};
use pxl_spec::space::pvf3::ivec;

use super::*;
use crate::global::serial;
use crate::layer::terrain::store::level_pages;

/// The levels the view draws, as a window this wide would give.
const VIEW: std::ops::Range<u32> = 0..4;

fn page(level: u32, x: i64, y: i64) -> LevelPage {
    LevelPage::new(level, ivec::gather(x, y, 0))
}

/// A page made resident, its lanes empty: nothing here reads the terrain,
/// only whether a page is in the table.
fn resident(p: LevelPage) {
    let lane = || std::borrow::Cow::Owned(Vec::new());
    let terrain = Terrain {
        ground: Ground {
            land: lane(),
            water: lane(),
            structure: lane(),
            transport: lane(),
        },
        ele: std::borrow::Cow::Owned(Vec::new()),
    };
    level_pages(p.level).set(p.page, Tile { terrain });
}

fn empty(levels: std::ops::Range<u32>) {
    for level in levels {
        for key in level_pages(level).keys().collect::<Vec<_>>() {
            level_pages(level).clear(key);
        }
    }
}

#[test]
fn a_page_whose_children_are_all_held_is_folded_rather_than_fetched() {
    let _g = serial();
    empty(0..3);
    let p = page(1, 2, 3);
    assert_eq!(resolve(p, &VIEW), Resolved::Missing);
    for (k, c) in p.children().iter().enumerate() {
        resident(*c);
        if k + 1 < 4 {
            assert_eq!(resolve(p, &VIEW), Resolved::Missing, "three of four is not a fold");
        }
    }
    assert_eq!(resolve(p, &VIEW), Resolved::Foldable);
    resident(p);
    assert_eq!(resolve(p, &VIEW), Resolved::Exact, "the page itself beats folding it");
    empty(0..3);
}

#[test]
fn a_page_with_something_coarser_over_it_is_covered_and_its_fetch_is_refinement() {
    let _g = serial();
    empty(0..3);
    let p = page(0, 6, 8);
    assert_eq!(resolve(p, &VIEW), Resolved::Missing);
    resident(p.parent());
    assert_eq!(resolve(p, &VIEW), Resolved::Covered);
    assert_eq!(ancestor(p, &VIEW), Some(p.parent()));
    assert!(covered(p, &VIEW));
    empty(0..3);
}

#[test]
fn coverage_is_looked_for_past_the_parent() {
    let _g = serial();
    empty(0..4);
    let p = page(0, 6, 8);
    let up = p.parent().parent();
    resident(up);
    assert_eq!(ancestor(p, &VIEW), Some(up), "a grandparent draws this ground too");
    assert_eq!(resolve(p, &VIEW), Resolved::Covered);
    empty(0..4);
}
