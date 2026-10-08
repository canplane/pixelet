// pxl-geo-bake/terrain/absent.test.rs

use super::*;
use pxl_spec::space::coord::pvf3::ivec::gather;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::LEAF_ZOOM;

/// One shard, at an odd index, so its west edge falls inside every coarser
/// zoom's tile.
fn absence() -> (Absence, Tile, Tile) {
    let inside = Tile::new(LEAF_ZOOM, gather(11392, 3200, 0));
    let west = Tile::new(LEAF_ZOOM, gather(11391, 3200, 0));
    (Absence::new(Coverage::new([[89, 25]])), inside, west)
}

#[test]
fn an_absent_tile_is_the_fold_of_the_absent_tiles_under_it() {
    let (absence, inside, west) = absence();
    for p in [inside, west] {
        for zoom in (GROUND.top()..LEAF_ZOOM).rev() {
            let p = Tile::over(zoom, p.first_leaf());
            let children = p.children().map(|c| absence.at(c));
            let folded = Terrain::fold_tile(p, |q| p.children().iter().position(|&c| c == q).map(|i| children[i].as_ref())).unwrap();
            assert_eq!(absence.at(p).ground, folded.ground, "z{zoom}");
        }
    }
}

#[test]
fn inside_the_coverage_absence_is_sea_and_past_it_unknown() {
    let (absence, inside, west) = absence();
    assert_eq!(absence.at(inside).ground.cells[0], ground::water);
    assert_eq!(absence.at(west).ground.cells[0], sentinel::unknown);
}
