// pxl-geo-bake/demand/terrain.rs

use super::job::{self, Job, Publication, Runner};
use super::{Dataset, Failure, Source as Producer};
use crate::terrain::leaf::{bake_ocean, ocean_samples, Coverage};
use crate::terrain::pyramid::neighbourhood;
use pxl_spec::bake::constant::{LEVEL0_ZOOM, PAGE_TOP_LEVEL};
use pxl_spec::bake::quadkey::index_of;
use pxl_spec::space::pvf3::ivec::IVec;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub(super) struct Source {
    coverage: Coverage,
    raw_ocean: Vec<u8>,
}
impl Source {
    pub fn new() -> Result<Self, String> {
        let coverage = Coverage::of_source();
        let raw_ocean = bake_ocean(&ocean_samples(&coverage))?;
        Ok(Self { coverage, raw_ocean })
    }
}

pub(super) async fn resolve(data: Arc<Dataset>, runner: Arc<Runner>, name: &str, top: u32) -> Result<String, Failure> {
    if name == "_ocean" {
        let d = data.clone();
        data.cache
            .once(name.into(), move |_deadline| async move {
                let Producer::Terrain(source) = &d.source else { unreachable!() };
                d.cache.publish("_ocean", Some(&source.raw_ocean)).map_err(Failure::from)
            })
            .await?;
        return Ok(name.into());
    }
    let (zoom, page) = index_of(name).filter(|(z, _)| LEVEL0_ZOOM - *z <= PAGE_TOP_LEVEL).ok_or(Failure::BadName)?;
    let level = LEVEL0_ZOOM - zoom;
    if level > top {
        return Err(Failure::Unavailable);
    }
    fault(data, runner, level, page).await?;
    Ok(format!("{level}/{page}"))
}

fn fault(data: Arc<Dataset>, runner: Arc<Runner>, level: u32, page: IVec) -> Pin<Box<dyn Future<Output = Result<(), Failure>> + Send>> {
    Box::pin(async move {
        let name = format!("{level}/{page}");
        let d = data.clone();
        data.cache
            .once(name.clone(), move |deadline| async move {
                let Producer::Terrain(source) = &d.source else { unreachable!() };
                if !source.coverage.meets(level, page) {
                    return d.cache.publish(&name, None).map_err(Failure::from);
                }
                if level > 0 {
                    // Dependencies do not occupy execution slots. Traverse one
                    // at a time so a coarse request cannot fan out a task tree.
                    for below in neighbourhood(page, level) {
                        tokio::time::timeout_at(deadline, fault(d.clone(), runner.clone(), level - 1, below))
                            .await
                            .map_err(|_| Failure::Limited("demand dependency deadline"))??;
                    }
                    d.cache.publish("_ocean", Some(&source.raw_ocean))?;
                }
                runner
                    .run(
                        Job::Terrain {
                            page,
                            level,
                            cache: d.cache.root.clone(),
                        },
                        d.clone(),
                        deadline,
                        Publication::Packet(name),
                    )
                    .await
            })
            .await
    })
}

pub(super) fn execute(page: u64, level: u32, cache: &std::path::Path, out: &std::path::Path, seconds: u64) -> Result<(), String> {
    use crate::terrain::{leaf, pyramid::fold_stored, repository::TerrainRepository, source::SourceStats, tile_set::TileSet};
    use crate::util::{pack, unpack, Level};
    use pxl_spec::bake::format::{decode_terrain, encode_terrain};
    let packet = if level == 0 {
        let mut client = job::connect(seconds)?;
        if !leaf::has_land(&mut client, &[page]).map_err(|e| e.to_string())?[0] {
            None
        } else {
            let repository = TerrainRepository::prepare(&mut client).map_err(|e| e.to_string())?;
            Some(leaf::bake_leaf(&repository, &mut client, page, &mut SourceStats::default(), Level::Live)?.1)
        }
    } else {
        let ocean = decode_terrain(&unpack(&std::fs::read(cache.join("_ocean")).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?).map_err(|e| format!("{e:?}"))?;
        fold_stored(&TileSet::new(cache), page, level, &ocean)?
            .map(|t| {
                let raw = encode_terrain(&t.ele, &t.ground).map_err(|e| format!("{e:?}"))?;
                pack(&raw, Level::Live).map_err(|e| e.to_string())
            })
            .transpose()?
    };
    job::write_packet(out, packet.as_deref())
}
