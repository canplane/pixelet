// pxl-geo-bake/demand/division.rs

use super::job;
use pxl_spec::bake::division_tile::DivisionTile;
use pxl_spec::space::layer::tile::Tile;

pub(super) fn execute(p: Tile, cache: &std::path::Path, out: &std::path::Path, seconds: u64) -> Result<(), String> {
    super::pyramid::execute::<DivisionTile>(
        p,
        cache,
        out,
        || {
            let coverage = crate::coverage::Coverage::of_source()?;
            let (_, mut client) = job::connect(&[&crate::division::READS], &coverage.around(p.zoom, p.tile), seconds)?;
            crate::division::leaf(&mut client, p.tile)
        },
        &DivisionTile::default(),
    )
}
