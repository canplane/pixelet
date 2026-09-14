// @pxl/shared/spec/space/sparse_tree/tree.test.ts

use core::cell::RefCell;
use std::rc::Rc;

use xpute_core::ds::buddy::{BuddyAllocator, BuddyOptions};
use xpute_core::ds::typed_alloc::buddy_alloc;

use super::*;
use crate::spec::space::pvf3::pvf3;
use crate::spec::space::sparse_tree::tree as sparse_tree;

const LAYOUT: SparseTreeLayout = SparseTreeLayout { dim: 3, min: -3, max: 6 };

fn at(x: f64, y: f64, z: f64) -> PVF3 {
    let mut out = PVF3::default();
    *pvf3::gather(&mut out, pvf3::from_real(x), pvf3::from_real(y), pvf3::from_real(z))
}

#[test]
fn sparse_tree_retype_widening_the_leaf_lane_carries_every_value_across() {
    let mut tree = sparse_tree::alloc::<u8>(LAYOUT, 64, None);
    sparse_tree::write(&mut tree, &at(3.0, 4.0, 5.0), 200, 0);
    sparse_tree::write(&mut tree, &at(10.125, 1.0, 0.0), 7, LAYOUT.min);
    let mut tree = sparse_tree::retype::<u8, u16>(tree);
    assert_eq!(sparse_tree::read(&tree, &at(3.5, 4.5, 5.5)), 200);
    assert_eq!(sparse_tree::read(&tree, &at(10.125, 1.0, 0.0)), 7);
    sparse_tree::write(&mut tree, &at(20.0, 20.0, 20.0), 300, 0);
    assert_eq!(sparse_tree::read(&tree, &at(20.0, 20.0, 20.0)), 300);
}

#[test]
fn sparse_tree_compact_a_pooled_tree_hands_its_old_lanes_back() {
    let pool = Rc::new(RefCell::new(
        BuddyAllocator::new(BuddyOptions {
            min_block_bytes: Some(1 << 12),
            region_bytes: Some(1 << 20),
            max_idle_regions: Some(1),
            poison: Some(false),
            bypass: Some(false),
            ..Default::default()
        })
        .unwrap(),
    ));
    let mut tree = sparse_tree::alloc::<u8>(LAYOUT, 64, Some(Box::new(buddy_alloc(pool.clone()))));
    for x in 0..16 {
        sparse_tree::write(&mut tree, &at(x as f64, 0.0, 0.0), 1, 0);
    }
    sparse_tree::compact(&mut tree);
    assert_eq!(pool.borrow().stat().live_blocks, 2, "only the compacted heap and data remain borrowed");
    sparse_tree::free(&mut tree);
    assert_eq!(pool.borrow().stat().live_blocks, 0);
}
