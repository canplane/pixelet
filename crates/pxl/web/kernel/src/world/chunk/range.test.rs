// pxl-kernel/world/chunk/range.test.rs

use pxl_spec::space::pvf3::ivec;

use super::*;

fn at(x: i64, y: i64) -> IVec {
    ivec::gather(x, y, 0)
}

#[test]
fn range_anchor_of_snaps_to_the_chunk_not_to_the_page() {
    assert_eq!(range_anchor_of(at(1100, 70)), at(1088, 64));
    assert_eq!(range_anchor_of(at(-1, -1)), at(-64, -64));
}
