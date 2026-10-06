// pxl-spec/space/hierarchy.test.rs

use super::*;

/// A child's parent and slot give it back, in any number of dimensions.
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

/// Several steps are one step taken as many times.
#[test]
fn several_steps_are_one_step_taken_as_many_times() {
    for at in [[5i64, -3], [-1, -1], [0, 8], [-9, 17]] {
        for steps in 0..5 {
            let mut up = at;
            for _ in 0..steps {
                up = parent_of(up);
            }
            assert_eq!(ancestor_of(at, steps), up);
            let mut first = at;
            for _ in 0..steps {
                first = child_of(first, 0);
            }
            assert_eq!(first_descendant_of(at, steps), first);
            assert_eq!(ancestor_of(first_descendant_of(at, steps), steps), at);
        }
    }
}

/// Two nodes overlap exactly when one lies under the other.
#[test]
fn nodes_share_ground_only_where_one_lies_under_the_other() {
    let at = (20, [3i64, -2]);
    assert!(share(at, at));
    for steps in 1..4u8 {
        let first = (20 + steps, first_descendant_of(at.1, u32::from(steps)));
        assert!(share(at, first) && share(first, at));
        let beside = (20 + steps, first_descendant_of([4, -2], u32::from(steps)));
        assert!(!share(at, beside) && !share(beside, at));
        assert!(share((20 - steps, ancestor_of(at.1, u32::from(steps))), at));
    }
}

/// Walk order inverts, and keeps a coarser node's leaves in one run.
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
