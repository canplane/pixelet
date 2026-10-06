// pxl-geo-bake/terrain/absent.test.rs

use super::*;

/// One shard, at an odd index, so its west edge falls inside every coarser
/// zoom's tile.
fn absence() -> (Absence, Tile) {
    let leaf = pxl_spec::space::coord::pvf3::ivec::gather(11392, 3200, 0);
    (Absence::new(Coverage::new([[89, 25]])), Tile::over(LEAF_ZOOM, leaf))
}

#[test]
fn an_absent_tile_is_the_fold_of_the_absent_tiles_under_it() {
    let (absence, leaf) = absence();
    for zoom in (LEAF_ZOOM - 10..LEAF_ZOOM).rev() {
        let p = Tile::over(zoom, leaf.first_leaf());
        let children = p.children().map(|c| absence.at(c));
        let folded = Terrain::fold_tile(p, |q| p.children().iter().position(|&c| c == q).map(|i| children[i].as_ref())).unwrap();
        assert_eq!(absence.at(p).ground, folded.ground, "z{zoom}");
    }
}

#[test]
fn inside_the_coverage_absence_is_sea_and_past_it_void() {
    let (absence, leaf) = absence();
    assert_eq!(absence.at(leaf).ground.cells[0], ground::water);
    let west = Tile::new(LEAF_ZOOM, pxl_spec::space::coord::pvf3::ivec::gather(11391, 3200, 0));
    assert_eq!(absence.at(west).ground.cells[0], sentinel::void);
}
