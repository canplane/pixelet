// pxl-geo-bake/demand/terrain.rs

use super::job;
use crate::coverage::Coverage;
use crate::terrain::absent::Absence;
use pxl_spec::space::coord::zoom::SHARD_ZOOM;
use pxl_spec::space::layer::tile::Tile;

/// Each tile's rows the deck solve read and each structure it solved, kept
/// beside the tiles of the source they came from.
pub(super) const SOURCE: &str = ".source";

/// The shards past the coverage, beside the tiles of the source they came from.
const COAST: &str = ".coast";

pub(super) fn execute(p: Tile, cache: &std::path::Path, out: &std::path::Path, seconds: u64) -> Result<(), String> {
    use crate::terrain::{leaf, repository::TerrainRepository};
    use pxl_spec::bake::format::TerrainTile;
    super::pyramid::execute::<TerrainTile>(
        p,
        cache,
        out,
        || {
            let (_, mut client) = job::connect(&leaf::READS, &Coverage::of_source()?.around(p.zoom, p.tile), seconds)?;
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
        &absence(p, cache, seconds)?,
    )
}

/// The coast a fold of `p` reads is baked first, once a source.
fn absence(p: Tile, cache: &std::path::Path, seconds: u64) -> Result<Absence, String> {
    use crate::pyramid::TileSet;
    use crate::terrain::{coast, leaf};
    let coverage = Coverage::of_source()?;
    let set = TileSet::new(cache.join(COAST));
    let beyond = coverage.beyond();
    if p.zoom < SHARD_ZOOM && beyond.iter().any(|&s| !set.path(s).exists()) {
        let (_, mut client) = job::connect(&leaf::READS, &coverage.shards().collect(), seconds)?;
        coast::bake_missing(&mut client, &set, &beyond)?;
    }
    Ok(Absence::new(coverage.clone(), coast::read(&set, &coverage)?))
}
