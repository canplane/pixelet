// pxl-spec/space/indexer.test.rs

use super::*;
use crate::space::pyramid::LEAF_Z;

#[test]
fn a_leaf_tile_s_index_and_base_round_trip_on_all_three_axes() {
    let addr = ivec::gather(-1, 2049, 1801);
    let at = idx(LEAF_Z, addr);
    assert_eq!(ivec::repr(at), "-1:4:3");
    assert_eq!(ivec::repr(from_idx(LEAF_Z, at)), "-512:2048:1536");
}
