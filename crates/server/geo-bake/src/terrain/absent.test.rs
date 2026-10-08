// pxl-geo-bake/terrain/absent.test.rs

use super::*;
use pxl_spec::space::coord::pvf3::ivec::gather;
use pxl_spec::space::layer::tile::LEAF_ZOOM;

/// One shard, and a leaf on either side of its west edge.
fn absence() -> (Absence, Tile, Tile) {
    let inside = Tile::new(LEAF_ZOOM, gather(11392, 3200, 0));
    let west = Tile::new(LEAF_ZOOM, gather(11391, 3200, 0));
    (Absence::new(Coverage::new([[89, 25]])), inside, west)
}

#[test]
fn inside_the_coverage_absence_is_sea_and_past_it_unknown() {
    let (absence, inside, west) = absence();
    assert_eq!(absence.at(inside).ground.cells[0], ground::water);
    assert_eq!(absence.at(west).ground.cells[0], sentinel::unknown);
}
