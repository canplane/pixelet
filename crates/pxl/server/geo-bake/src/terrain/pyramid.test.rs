// pxl-geo-bake/terrain/pyramid.test.rs

use std::collections::HashMap;

use pxl_spec::bake::constant::PAGE_CELL_SPAN;

use super::*;

const SPAN: usize = PAGE_CELL_SPAN as usize;
const PADDED: usize = ELE_PADDED_SPAN as usize;

fn at(x: i64, y: i64) -> IVec {
    ivec::gather(x, y, 0)
}

fn uniform(land: u8, height: i16) -> Terrain {
    let plane = |v: u8| vec![v; SPAN * SPAN].into();
    Terrain {
        ground: Ground {
            land: plane(land),
            water: plane(0),
            structure: plane(0),
            transport: plane(0),
        },
        ele: vec![height; PADDED * PADDED].into(),
    }
}

#[test]
fn a_parent_is_its_child_shifted_right_a_floor_on_negatives() {
    assert_eq!(parent_of(at(0, 1)), at(0, 0));
    assert_eq!(parent_of(at(3, 2)), at(1, 1));
    assert_eq!(parent_of(at(-1, -1)), at(-1, -1));
    assert_eq!(parent_of(at(-2, -3)), at(-1, -2));
}

#[test]
fn a_level_has_one_parent_per_block_of_children() {
    let children = [at(0, 0), at(1, 0), at(0, 1), at(1, 1), at(2, 0), at(-1, 0)];
    // Key order is the packed word's, where a negative X sorts high.
    let mut want = vec![at(-1, 0), at(0, 0), at(1, 0)];
    want.sort_unstable();
    assert_eq!(parents(children), want);
}

#[test]
fn the_neighbourhood_puts_the_parents_own_four_in_the_middle() {
    let around = neighbourhood(at(5, 7), 1);
    assert_eq!([around[5], around[6], around[9], around[10]], [at(10, 14), at(11, 14), at(10, 15), at(11, 15)]);
    assert_eq!(around[0], at(9, 13));
    assert_eq!(around[15], at(12, 16));
}

#[test]
fn the_neighbourhood_wraps_x_across_the_seam() {
    // The westmost parent at level 1 has the westmost level-0 pages as its
    // children, and the page west of those is the eastmost.
    let west = -pages_across(1) / 2;
    let around = neighbourhood(at(west, 0), 1);
    assert_eq!(ivec::X(around[5]), -pages_across(0) / 2);
    assert_eq!(ivec::X(around[4]), pages_across(0) / 2 - 1);
}

#[test]
fn each_child_lands_in_its_quarter_and_a_missing_one_is_the_ocean() {
    let ocean = uniform(99, 0);
    let pages: HashMap<IVec, Terrain> = [(at(0, 0), uniform(1, 10)), (at(1, 0), uniform(2, 10)), (at(0, 1), uniform(3, 10))].into_iter().collect();
    let parent = fold_parent(at(0, 0), 1, |p| pages.get(&p).map(|t| &t.ground), |p| pages.get(&p).map(|t| &*t.ele), &ocean).expect("a parent");
    let half = SPAN / 2;
    let land = &parent.ground.land;
    assert_eq!(land[0], 1);
    assert_eq!(land[half], 2);
    assert_eq!(land[half * SPAN], 3);
    assert_eq!(land[half * SPAN + half], 99, "the missing north-east child");
}

#[test]
fn the_parents_elevation_reads_its_neighbours_not_only_its_children() {
    // Children at 10, every other page absent and so the ocean's 0: the
    // parent's middle is 10 and its outermost skirt, which lies over the
    // neighbours alone, is 0.
    let ocean = uniform(99, 0);
    let pages: HashMap<IVec, Terrain> = [at(0, 0), at(1, 0), at(0, 1), at(1, 1)].into_iter().map(|p| (p, uniform(1, 10))).collect();
    let parent = fold_parent(at(0, 0), 1, |p| pages.get(&p).map(|t| &t.ground), |p| pages.get(&p).map(|t| &*t.ele), &ocean).expect("a parent");
    assert_eq!(parent.ele[(PADDED / 2) * PADDED + PADDED / 2], 10);
    assert_eq!(parent.ele[0], 0);
}

#[test]
fn a_parent_with_no_child_does_not_exist_whatever_its_neighbours_hold() {
    let ocean = uniform(99, 0);
    // Land beside the parent's block, none in it.
    let pages: HashMap<IVec, Terrain> = [(at(2, 0), uniform(1, 10)), (at(-1, 1), uniform(1, 10))].into_iter().collect();
    assert!(fold_parent(at(0, 0), 1, |p| pages.get(&p).map(|t| &t.ground), |p| pages.get(&p).map(|t| &*t.ele), &ocean).is_none());
}
