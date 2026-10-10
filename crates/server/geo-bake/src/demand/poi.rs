// pxl-geo-bake/demand/poi.rs

use super::job;
use pxl_spec::bake::poi_tile::PoiTile;
use pxl_spec::space::layer::tile::Tile;

pub(super) fn execute(p: Tile, cache: &std::path::Path, out: &std::path::Path, seconds: u64) -> Result<(), String> {
    super::pyramid::execute::<PoiTile>(
        p,
        cache,
        out,
        || {
            let coverage = crate::coverage::Coverage::of_source()?;
            Ok(crate::poi::leaves(&mut job::connect(&[&crate::poi::READS], &coverage.around(p.tile), seconds)?.1, &coverage, Some(p.tile))?.remove(&p.tile))
        },
        &PoiTile::default(),
    )
}
