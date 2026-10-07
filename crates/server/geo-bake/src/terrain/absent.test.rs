// pxl-geo-bake/terrain/absent.test.rs

use super::*;
use pxl_spec::space::coord::pvf3::ivec::gather;
use pxl_spec::space::layer::tile::LEAF_ZOOM;

/// One shard, at an odd index, so its west edge falls inside every coarser
/// zoom's tile; each shard past it is land in its north half and sea in its
/// south, so a fold meets both.
fn absence() -> (Absence, Tile) {
    let coverage = Coverage::new([[89, 25]]);
    let half = |i: u32| if GROUND.in_tile(i)[1] < TILE_CELLS / 2 { sentinel::unknown } else { ground::water };
    let coast = coverage
        .beyond()
        .into_iter()
        .map(|p| {
            let cells: Vec<_> = (0..TILE_CELLS * TILE_CELLS).map(half).collect();
            (p, surface(Ground { cells: cells.into() }))
        })
        .collect();
    (Absence::new(coverage, coast), Tile::over(LEAF_ZOOM, gather(11392, 3200, 0)))
}

#[test]
fn an_absent_tile_is_the_fold_of_the_absent_tiles_under_it() {
    let (absence, leaf) = absence();
    let west = Tile::new(LEAF_ZOOM, gather(11391, 3200, 0));
    for p in [leaf, west] {
        for zoom in (GROUND.top()..LEAF_ZOOM).rev() {
            let p = Tile::over(zoom, p.first_leaf());
            let children = p.children().map(|c| absence.at(c));
            let folded = Terrain::fold_tile(p, |q| p.children().iter().position(|&c| c == q).map(|i| children[i].as_ref())).unwrap();
            assert_eq!(absence.at(p).ground, folded.ground, "z{zoom}");
        }
    }
}

#[test]
fn inside_the_coverage_absence_is_sea_and_past_it_the_coast() {
    let (absence, leaf) = absence();
    assert_eq!(absence.at(leaf).ground.cells[0], ground::water);
    let west = Tile::new(LEAF_ZOOM, gather(11391, 3200, 0));
    let shard = Tile::over(SHARD_ZOOM, west.first_leaf());
    let expect = absence.coast[&shard].ground.cells[GROUND.cell_of(SHARD_ZOOM, &GROUND.cell_at(west, 0).base()).index() as usize];
    assert_eq!(absence.at(west).ground.cells[0], expect);
}
