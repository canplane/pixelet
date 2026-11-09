// pxl-geo-bake/oven/pyramid.rs

//! A pyramid layer baked a tile at a time on demand. Missing finer tiles are baked one at a
//! time, so a coarse request cannot fan out into a tree of tasks.

use super::job::{self, Job, Runner};
use super::{Dataset, Failure, Kind};
use crate::pyramid::{fold_parent, Absent, Base, TileSet};
use crate::util::{pack, Level};
use fnv::FnvHashMap;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::layer::quadrant::Quadrant;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use tokio::time;

pub(super) async fn resolve(
    data: Arc<Dataset>,
    runner: Arc<Runner>,
    kind: Kind,
    name: &str,
    top: Zoom,
) -> Result<Option<String>, Failure> {
    let p = Quadrant::of_key(name).ok_or(Failure::BadName)?;
    if p.zoom < top {
        return Err(Failure::Unavailable);
    }
    if p.zoom > kind.leaf() {
        return Ok(None);
    }
    bake_missing(data, runner, kind, p, true).await?;
    Ok(Some(p.key()))
}

fn bake_missing(
    data: Arc<Dataset>,
    runner: Arc<Runner>,
    kind: Kind,
    p: Quadrant,
    asked: bool,
) -> Pin<Box<dyn Future<Output = Result<(), Failure>> + Send>> {
    Box::pin(async move {
        let d = data.clone();
        data.cache
            .once(p.key(), asked, move |lease| async move {
                if !d.coverage.meets(p.zoom, p.at) {
                    return d.cache.publish(&p.key(), None).map_err(Failure::from);
                }
                if p.zoom < kind.leaf() {
                    for below in (d.sources)(p) {
                        time::timeout_at(
                            lease.deadline,
                            bake_missing(d.clone(), runner.clone(), kind, below, false),
                        )
                        .await
                        .map_err(|_| Failure::Limited("oven dependency deadline"))??;
                    }
                }
                let job = Job {
                    kind,
                    id: p.id(),
                    cache: d.cache.root.clone(),
                };
                runner.run(job, d.clone(), lease, p.key()).await
            })
            .await
    })
}

pub(super) fn execute<L: Base>(
    p: Quadrant,
    cache: &Path,
    out: &Path,
    leaf: impl FnOnce() -> Result<Option<L>, String>,
    absent: &impl Absent<L>,
) -> Result<(), String> {
    let tile = if p.zoom == L::LEAF {
        leaf()?
    } else {
        let set = TileSet::new(cache);
        let mut sources = FnvHashMap::default();
        for q in L::sources(p) {
            if let Some(tile) = set.read::<L>(q)? {
                sources.insert(q, tile);
            }
        }
        fold_parent(p, |q| sources.get(&q), absent)
    };
    let pkt = tile
        .map(|tile| pack(&tile.encode(p)?, Level::Live).map_err(|e| e.to_string()))
        .transpose()?;
    job::write_pkt(out, pkt.as_deref())
}
