// pxl-spec/space/sparse_tree.test.rs

use super::*;

fn any(kids: &[u16], _: &[bool], _: Zoom) -> u16 {
    kids.iter().copied().max().unwrap_or(0)
}

#[test]
fn a_write_undone_leaves_the_tree_as_it_was_on_either_side_of_zero() {
    let mut tree = SparseTree::<u16>::default();
    for at in [[3, -4, 5], [-1, -1, -1], [0, 0, 0]] {
        assert!(tree.set(27, at, 7, &any));
        assert_eq!(tree.get(27, at).data, 7);
        assert!(tree.set(27, at, 0, &any));
        assert!(tree.is_empty());
    }
}

#[test]
fn eight_equal_children_are_their_parent() {
    let mut tree = SparseTree::<u16>::default();
    for k in 0..8 {
        tree.set(20, child_of([5, -6, 7], k), 3, &any);
    }
    let node = tree.get(20, child_of([5, -6, 7], 0));
    assert_eq!((node.zoom, node.at, node.data, node.branch()), (19, [5, -6, 7], 3, false));
}
