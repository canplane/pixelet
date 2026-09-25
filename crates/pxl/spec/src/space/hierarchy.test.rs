// pxl-spec/space/hierarchy.test.rs

use super::*;

/// The hierarchy is one algebra in any number of dimensions: a child's
/// parent and slot give it back, and a parent's children are the 2^D nodes
/// whose parent it is.
#[test]
fn a_child_is_its_parent_and_slot_in_two_and_three_dimensions() {
    for at in [[5i64, -3], [-1, -1], [0, 8]] {
        assert_eq!(child_of(parent_of(at), slot_of(at)), at);
        assert!((0..4).all(|k| parent_of(child_of(at, k)) == at && slot_of(child_of(at, k)) == k));
    }
    for at in [[5i64, -3, 2], [-1, -1, -1], [0, 8, 7]] {
        assert_eq!(child_of(parent_of(at), slot_of(at)), at);
        assert!((0..8).all(|k| parent_of(child_of(at, k)) == at && slot_of(child_of(at, k)) == k));
    }
}

/// A scale counts, steps and measures as one power of two: a side is the
/// count of the unit in it, stepping up and back is the identity, and a count
/// across several levels is the product of the counts across each.
#[test]
fn a_scale_is_one_power_of_two_however_it_is_read() {
    let unit = Scale::of(0);
    for s in [Scale::of(-3), unit, Scale::of(6), Scale::of(9)] {
        assert_eq!(s.up(2).down(2), s);
        assert_eq!(s.up(3).above(s), 3);
        assert_eq!(Scale::of_side(s.side()), s);
        assert_eq!(s.count_in(s.up(2)) * s.up(2).count_in(s.up(5)), s.count_in(s.up(5)));
    }
    assert_eq!(unit.count_in(Scale::of(9)) as f64, Scale::of(9).side());
    assert_eq!(Scale::of(-3).side(), 0.125);
}

/// A walk's order is its inverse's, and it keeps a coarser node's leaves
/// together: every leaf of one node falls in one run, the node's own order
/// shifted up by the levels below it.
#[test]
fn a_walk_order_is_invertible_and_keeps_a_node_s_leaves_in_one_run() {
    for at in [[0u32, 0, 0], [5, 3, 7], [63, 0, 31], [12, 40, 9]] {
        assert_eq!(at_walk_order::<3>(walk_order(at, 6), 6), at);
        let node = walk_order(at.map(|v| v >> 2), 4);
        assert_eq!(walk_order(at, 6) >> (3 * 2), node, "{at:?} lies in its node's run");
    }
    let mut order: Vec<u64> = (0..16u32).flat_map(|y| (0..16u32).map(move |x| walk_order([x, y], 4))).collect();
    order.sort_unstable();
    order.dedup();
    assert_eq!(order, (0..256).collect::<Vec<_>>(), "a walk visits every node once");
}
