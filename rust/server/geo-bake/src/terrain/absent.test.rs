// pxl-geo-bake/terrain/absent.test.rs

use super::*;
use pxl_spec::space::coord::zoom::{LEAF_ZOOM, SHARD_ZOOM};
use pxl_spec::space::hierarchy::tree::first_descendant_of;

/// The westmost shard's first leaf, and the leaf west of it, which no shard holds.
fn absence() -> (Absence, Quadrant, Quadrant) {
    let coverage = Coverage::of_source().expect("the source's coverage");
    let shard = coverage.shards().next().expect("a shard");
    let [x, y] = first_descendant_of(shard.at, LEAF_ZOOM - SHARD_ZOOM);
    let inside = Quadrant::new(LEAF_ZOOM, [x, y]);
    let west = Quadrant::new(LEAF_ZOOM, [x - 1, y]);
    (Absence::new(coverage), inside, west)
}

#[test]
fn absence_sea_or_unknown() {
    let (absence, inside, west) = absence();
    assert_eq!(absence.at(inside).cells[0], ground::water);
    assert_eq!(absence.at(west).cells[0], sentinel::unknown);
}
