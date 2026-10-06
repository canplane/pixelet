// pxl-geo-bake/demand/pyramid.rs

//! A pyramid layer baked a tile at a time on demand. Lower tiles missing are baked
//! one at a time, so a coarse request cannot fan out a task tree.

use super::job::{self, Job, Publication, Runner};
use super::{Dataset, Failure, Kind};
use crate::pyramid::{fold_stored, Absent, Layer, TileSet};
use crate::util::{pack, Level};
use pxl_spec::bake::quadkey::tile_of_name;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::layer::tile::{Tile, LEAF_ZOOM};
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;

pub(super) async fn resolve(data: Arc<Dataset>, runner: Arc<Runner>, kind: Kind, name: &str, top: Zoom) -> Result<Option<String>, Failure> {
    let p = tile_of_name(name).ok_or(Failure::BadName)?;
    if p.zoom < top {
        return Err(Failure::Unavailable);
    }
    bake_missing(data, runner, kind, p, true).await?;
    Ok(Some(key(p)))
}

fn key(p: Tile) -> String {
    format!("z{}/{}", p.zoom, p.tile)
}

fn bake_missing(data: Arc<Dataset>, runner: Arc<Runner>, kind: Kind, p: Tile, asked: bool) -> Pin<Box<dyn Future<Output = Result<(), Failure>> + Send>> {
    Box::pin(async move {
        let d = data.clone();
        data.cache
            .once(key(p), asked, move |lease| async move {
                if !d.source.holds(p) {
                    return d.cache.publish(&key(p), None).map_err(Failure::from);
                }
                if p.zoom < LEAF_ZOOM {
                    for below in d.source.sources(p) {
                        tokio::time::timeout_at(lease.deadline, bake_missing(d.clone(), runner.clone(), kind, below, false))
                            .await
                            .map_err(|_| Failure::Limited("demand dependency deadline"))??;
                    }
                }
                let job = Job::Tile {
                    kind,
                    subject: p.subject(),
                    cache: d.cache.root.clone(),
                };
                runner.run(job, d.clone(), lease, Publication::Packet(key(p))).await
            })
            .await
    })
}

pub(super) fn execute<L: Layer>(p: Tile, cache: &Path, out: &Path, leaf: impl FnOnce() -> Result<Option<L>, String>, absent: &impl Absent<L>) -> Result<(), String> {
    let tile = if p.zoom == LEAF_ZOOM { leaf()? } else { fold_stored(&TileSet::new(cache), p, absent)? };
    let packet = tile.map(|tile| pack(&tile.encode()?, Level::Live).map_err(|e| e.to_string())).transpose()?;
    job::write_packet(out, packet.as_deref())
}
