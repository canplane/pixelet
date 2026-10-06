// pxl-geo-bake/demand/terrain.rs

use super::job;
use crate::coverage::Coverage;
use crate::terrain::absent::Absence;
use pxl_spec::space::layer::tile::Tile;

/// Each tile's rows the deck solve read and each structure it solved, kept
/// for the tiles around while the server runs.
pub(super) const SOURCE: &str = ".source";

pub(super) fn execute(p: Tile, cache: &std::path::Path, out: &std::path::Path, seconds: u64) -> Result<(), String> {
    use crate::terrain::{leaf, repository::TerrainRepository};
    use pxl_spec::bake::format::Terrain;
    super::pyramid::execute::<Terrain>(
        p,
        cache,
        out,
        || {
            let (_, mut client) = job::connect(&leaf::READS, &Coverage::of_source()?.around(p.tile), seconds)?;
            if !leaf::has_ground(&mut client, &[p.tile]).map_err(|e| e.to_string())?[0] {
                return Ok(None);
            }
            let raster = std::sync::Arc::new(crate::elevation::Raster::open(&mut client)?);
            let repo = TerrainRepository::prepare(&mut client, raster).map_err(|e| e.to_string())?;
            let memo = cache.join(SOURCE);
            let reader = &mut crate::deck::Reader {
                client: &mut client,
                repo: &repo,
                store: &mut crate::deck::source::Disk(&memo),
            };
            Ok(Some(leaf::bake_leaf(reader, p.tile)?.0))
        },
        &Absence::new(Coverage::of_source()?),
    )
}
