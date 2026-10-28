// pxl-spec/space/hierarchy/tree.test.rs

use super::*;

/// A child's parent and slot give it back, in any number of dimensions.
#[test]
fn child_parent_slot() {
    for at in [[5i32, -3], [-1, -1], [0, 8]] {
        assert_eq!(child_of(parent_of(at), slot_of(at)), at);
        assert!((0..4).all(|k| parent_of(child_of(at, k)) == at && slot_of(child_of(at, k)) == k));
    }
    for at in [[5i32, -3, 2], [-1, -1, -1], [0, 8, 7]] {
        assert_eq!(child_of(parent_of(at), slot_of(at)), at);
        assert!((0..8).all(|k| parent_of(child_of(at, k)) == at && slot_of(child_of(at, k)) == k));
    }
}

/// Several steps are one step taken as many times.
#[test]
fn steps_compose() {
    for at in [[5i32, -3], [-1, -1], [0, 8], [-9, 17]] {
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
fn overlap_is_nesting() {
    let at = (20, [3i32, -2]);
    assert!(share(at, at));
    for steps in 1..4u32 {
        let first = (20 + steps, first_descendant_of(at.1, steps));
        assert!(share(at, first) && share(first, at));
        let beside = (20 + steps, first_descendant_of([4, -2], steps));
        assert!(!share(at, beside) && !share(beside, at));
        assert!(share((20 - steps, ancestor_of(at.1, steps)), at));
    }
}
