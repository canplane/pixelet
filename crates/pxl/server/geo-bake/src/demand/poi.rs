// pxl-geo-bake/demand/poi.rs

use super::job;
use pxl_spec::bake::poi_tile::PoiTile;
use pxl_spec::space::pyramid::Tile;

pub(super) fn execute(p: Tile, cache: &std::path::Path, out: &std::path::Path, seconds: u64) -> Result<(), String> {
    super::pyramid::execute::<PoiTile>(
        p,
        cache,
        out,
        || Ok(crate::poi::leaves(&mut job::connect(&[&crate::poi::READS], seconds)?.1, &crate::coverage::Coverage::of_source()?, Some(p.tile))?.remove(&p.tile)),
        &PoiTile::default(),
    )
}
