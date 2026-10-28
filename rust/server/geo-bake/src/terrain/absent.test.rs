// pxl-geo-bake/terrain/absent.test.rs

use super::*;
use pxl_spec::space::coord::zoom::SHARD_ZOOM;
use pxl_spec::space::hierarchy::tree::first_descendant_of;
use pxl_spec_core::param::tile::LEAF_ZOOM;

/// The westmost shard's first leaf, and the leaf west of it, which no shard holds.
fn absence() -> (Absence, Tile, Tile) {
    let coverage = Coverage::of_source().expect("the source's coverage");
    let shard = coverage.shards().next().expect("a shard");
    let [x, y] = first_descendant_of(shard.at, LEAF_ZOOM - SHARD_ZOOM);
    let inside = Tile::new(LEAF_ZOOM, [x, y]);
    let west = Tile::new(LEAF_ZOOM, [x - 1, y]);
    (Absence::new(coverage), inside, west)
}

#[test]
fn inside_the_coverage_absence_is_sea_and_past_it_unknown() {
    let (absence, inside, west) = absence();
    assert_eq!(absence.at(inside).ground.cells[0], ground::water);
    assert_eq!(absence.at(west).ground.cells[0], sentinel::unknown);
}
