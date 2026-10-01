// pxl-geo-bake/demand/terrain.rs

use super::job;
use crate::terrain::leaf::{bake_ocean, ocean_samples, Coverage};
use pxl_spec::space::pyramid::LevelPage;

pub(super) struct Source {
    pub(super) coverage: Coverage,
    pub(super) raw_ocean: Vec<u8>,
}
impl Source {
    pub fn new() -> Result<Self, String> {
        let coverage = Coverage::of_source();
        let raw_ocean = bake_ocean(&ocean_samples(&coverage))?;
        Ok(Self { coverage, raw_ocean })
    }
}

/// A terrain page: a leaf baked from the database where it has land, a page
/// above folded with the ocean page for open water.
pub(super) fn execute(p: LevelPage, cache: &std::path::Path, out: &std::path::Path, seconds: u64) -> Result<(), String> {
    use crate::terrain::{leaf, repository::TerrainRepository, source::SourceStats};
    use crate::util::{unpack, Level};
    use pxl_spec::bake::format::{decode_terrain, Terrain};
    super::pyramid::execute::<Terrain>(
        p,
        cache,
        out,
        || {
            let mut client = job::connect(seconds)?;
            if !leaf::has_land(&mut client, &[p.page]).map_err(|e| e.to_string())?[0] {
                return Ok(None);
            }
            let raster = std::sync::Arc::new(crate::elevation::Raster::open(&mut client).map_err(|e| e.to_string())?);
            let repository = TerrainRepository::prepare(&mut client, raster).map_err(|e| e.to_string())?;
            let packed = leaf::bake_leaf(&repository, &mut client, p.page, &mut SourceStats::default(), Level::Live)?.1;
            decode_terrain(&unpack(&packed).map_err(|e| e.to_string())?).map(Some).map_err(|e| format!("{e:?}"))
        },
        || decode_terrain(&unpack(&std::fs::read(cache.join(leaf::OCEAN_KEY)).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?).map_err(|e| format!("{e:?}")),
    )
}
