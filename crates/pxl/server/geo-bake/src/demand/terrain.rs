// pxl-geo-bake/demand/terrain.rs

use super::{Dataset, Failure, Pool, Source as Producer};
use crate::terrain::leaf::{bake_leaf, bake_ocean, has_land, ocean_samples, Coverage};
use crate::terrain::pyramid::{fold_stored, neighbourhood};
use crate::terrain::tile_set::TileSet;
use crate::util::Level;
use pxl_spec::bake::constant::{LEVEL0_ZOOM, PAGE_TOP_LEVEL};
use pxl_spec::bake::format::{decode_terrain, encode_terrain, Terrain};
use pxl_spec::bake::quadkey::index_of;
use pxl_spec::space::pvf3::ivec::IVec;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub(super) struct Source {
    coverage: Coverage,
    ocean: Terrain,
    raw_ocean: Vec<u8>,
}
impl Source {
    pub fn new() -> Result<Self, String> {
        let coverage = Coverage::of_source();
        let raw_ocean = bake_ocean(&ocean_samples(&coverage))?;
        let ocean = decode_terrain(&raw_ocean).map_err(|e| format!("{e:?}"))?;
        Ok(Self { coverage, ocean, raw_ocean })
    }
}

pub(super) async fn resolve(data: Arc<Dataset>, pool: Arc<Pool>, name: &str, top: u32) -> Result<String, Failure> {
    if name == "_ocean" {
        let d = data.clone();
        data.cache
            .once(name.into(), async move {
                let Producer::Terrain(source) = &d.source else { unreachable!() };
                d.cache.publish("_ocean", Some(&source.raw_ocean))
            })
            .await?;
        return Ok(name.into());
    }
    let (zoom, page) = index_of(name).filter(|(z, _)| LEVEL0_ZOOM - *z <= PAGE_TOP_LEVEL).ok_or(Failure::BadName)?;
    let level = LEVEL0_ZOOM - zoom;
    if level > top {
        return Err(Failure::Unavailable);
    }
    fault(data, pool, level, page).await?;
    Ok(format!("{level}/{page}"))
}

fn fault(data: Arc<Dataset>, pool: Arc<Pool>, level: u32, page: IVec) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
    Box::pin(async move {
        let name = format!("{level}/{page}");
        let d = data.clone();
        data.cache
            .once(name.clone(), async move {
                let Producer::Terrain(source) = &d.source else { unreachable!() };
                if !source.coverage.meets(level, page) {
                    return d.cache.publish(&name, None);
                }
                if level > 0 {
                    let below: Vec<_> = neighbourhood(page, level).into_iter().map(|p| tokio::spawn(fault(d.clone(), pool.clone(), level - 1, p))).collect();
                    for task in below {
                        task.await.map_err(|e| e.to_string())??;
                    }
                }
                tokio::task::spawn_blocking(move || {
                    let Producer::Terrain(source) = &d.source else { unreachable!() };
                    if level == 0 {
                        let packed = pool.with(|w| {
                            if !has_land(&mut w.client, &[page]).map_err(|e| e.to_string())?[0] {
                                return Ok(None);
                            }
                            bake_leaf(&w.terrain, &mut w.client, page, &mut w.stats, Level::Live).map(|(_, packed)| Some(packed))
                        })?;
                        return d.cache.publish_stored(&name, packed.as_deref());
                    }
                    let raw = {
                        fold_stored(&TileSet::new(&d.cache.root), page, level, &source.ocean)?
                            .map(|t| encode_terrain(&t.ele, &t.ground).map_err(|e| format!("{e:?}")))
                            .transpose()?
                    };
                    d.cache.publish(&name, raw.as_deref())
                })
                .await
                .map_err(|e| e.to_string())?
            })
            .await
    })
}
