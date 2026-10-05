// pxl-spec/space/sparse_tree/tree.test.rs

use super::*;
use crate::space::pvf3::pvf3;
use crate::space::sparse_tree::tree as sparse_tree;

const LAYOUT: SparseTreeLayout = SparseTreeLayout { dim: 3, min: 27, max: 18 };

fn at(x: f64, y: f64, z: f64) -> Pvf3 {
    let _out = Pvf3::default();
    pvf3::gather(pvf3::from_real(x), pvf3::from_real(y), pvf3::from_real(z))
}

#[test]
fn sparse_tree_retype_widening_the_leaf_lane_carries_every_value_across() {
    let mut tree = sparse_tree::alloc::<u8>(LAYOUT, 64);
    sparse_tree::write(&mut tree, &at(3.0, 4.0, 5.0), 200, 24);
    sparse_tree::write(&mut tree, &at(10.125, 1.0, 0.0), 7, LAYOUT.min);
    let mut tree = sparse_tree::retype::<u8, u16>(tree);
    assert_eq!(sparse_tree::read(&tree, &at(3.5, 4.5, 5.5)), 200);
    assert_eq!(sparse_tree::read(&tree, &at(10.125, 1.0, 0.0)), 7);
    sparse_tree::write(&mut tree, &at(20.0, 20.0, 20.0), 300, 24);
    assert_eq!(sparse_tree::read(&tree, &at(20.0, 20.0, 20.0)), 300);
}

#[test]
#[should_panic(expected = "EOVERFLOW")]
fn sparse_tree_decode_refuses_a_blob_whose_count_overflows_u32_rather_than_wrapping_under_it() {
    let mut blob = vec![0u64; 4];
    blob[0] = 0xffff_ffff;
    let _ = decode::<u16>(&blob, SparseTreeLayout { dim: 3, min: 27, max: 21 });
}
