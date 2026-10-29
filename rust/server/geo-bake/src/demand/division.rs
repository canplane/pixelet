// pxl-geo-bake/demand/division.rs

use super::job;
use crate::coverage::Coverage;
use crate::division;
use pxl_spec::bake::tile::division::DivisionTile;
use pxl_spec::space::layer::tile::Tile;

pub(super) fn execute(
    p: Tile,
    cache: &std::path::Path,
    out: &std::path::Path,
    seconds: u64,
) -> Result<(), String> {
    super::pyramid::execute::<DivisionTile>(
        p,
        cache,
        out,
        || {
            let coverage = Coverage::of_source()?;
            let (_, mut client) =
                job::connect(&[&division::READS], &coverage.around(p.zoom, p.at), seconds)?;
            division::leaf(&mut client, p.at)
        },
        &DivisionTile::default(),
    )
}
