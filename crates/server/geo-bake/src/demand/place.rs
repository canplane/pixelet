// pxl-geo-bake/demand/place.rs

use super::job;
use pxl_spec::bake::tile::place::PlaceTile;
use pxl_spec::space::layer::tile::Tile;

pub(super) fn execute(p: Tile, cache: &std::path::Path, out: &std::path::Path, seconds: u64) -> Result<(), String> {
    super::pyramid::execute::<PlaceTile>(
        p,
        cache,
        out,
        || {
            let coverage = crate::coverage::Coverage::of_source()?;
            Ok(crate::place::leaves(&mut job::connect(&[&crate::place::READS], &coverage.around(p.zoom, p.at), seconds)?.1, &coverage, p)?.remove(&p))
        },
        &PlaceTile::default(),
    )
}
