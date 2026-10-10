// pxl-geo-bake/demand/terrain.rs

use super::job;
use crate::coverage::Coverage;
use crate::terrain::leaf::{bake_ocean, ocean_samples};
use pxl_spec::space::pyramid::Page;

/// Each page's rows the deck solve read and each structure it solved, kept
/// for the pages around while the server runs.
pub(super) const SOURCE: &str = ".source";

/// The page every absent terrain page is.
pub(super) fn ocean() -> Result<Vec<u8>, String> {
    bake_ocean(&ocean_samples(&Coverage::of_source()?))
}

pub(super) fn execute(p: Page, cache: &std::path::Path, out: &std::path::Path, seconds: u64) -> Result<(), String> {
    use crate::terrain::{leaf, repository::TerrainRepository};
    use crate::util::unpack;
    use pxl_spec::bake::format::{decode_terrain, Terrain};
    super::pyramid::execute::<Terrain>(
        p,
        cache,
        out,
        || {
            let (_, mut client) = job::connect(&leaf::READS, seconds)?;
            if !leaf::has_ground(&mut client, &[p.page]).map_err(|e| e.to_string())?[0] {
                return Ok(None);
            }
            let raster = std::sync::Arc::new(crate::elevation::Raster::open(&mut client).map_err(|e| e.to_string())?);
            let repo = TerrainRepository::prepare(&mut client, raster).map_err(|e| e.to_string())?;
            let memo = cache.join(SOURCE);
            let reader = &mut crate::deck::Reader {
                client: &mut client,
                repo: &repo,
                store: &mut crate::deck::source::Disk(&memo),
                memo: &memo.join("structure"),
            };
            Ok(Some(leaf::bake_leaf(reader, p.page)?.0))
        },
        || decode_terrain(&unpack(&std::fs::read(cache.join(leaf::OCEAN_KEY)).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?).map_err(|e| format!("{e:?}")),
    )
}
