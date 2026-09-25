// pxl-geo-bake/demand/pyramid.rs

//! A layer the dev server holds as a pyramid, a page at a time as pages are
//! wanted. A name is a page; a leaf is baked from the layer's source, and a
//! page above it is folded from the pages of the level below its layer reads,
//! which are faulted in first — one at a time, so a coarse request cannot fan out a
//! task tree — exactly as the offline bake would have made it
//! (pxl-geo-bake pyramid.rs). Past `top` nothing is answered, which is the
//! guard every layer shares.

use super::job::{self, Job, Publication, Runner};
use super::{Dataset, Failure, Kind};
use crate::pyramid::{fold_stored, Layer, PageSet};
use crate::util::{pack, Level};
use pxl_spec::bake::quadkey::page_of_name;
use pxl_spec::space::pyramid::LevelPage;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;

pub(super) async fn resolve(data: Arc<Dataset>, runner: Arc<Runner>, kind: Kind, name: &str, top: u32) -> Result<Option<String>, Failure> {
    if data.source.shared(name).is_some() {
        let (d, shared) = (data.clone(), name.to_owned());
        data.cache
            .once(name.into(), move |_deadline| async move {
                let name = shared;
                let bytes = d.source.shared(&name).expect("a shared object");
                d.cache.publish(&name, Some(bytes)).map_err(Failure::from)
            })
            .await?;
        return Ok(Some(name.into()));
    }
    let p = page_of_name(name).ok_or(Failure::BadName)?;
    if p.level > top {
        return Err(Failure::Unavailable);
    }
    fault(data, runner, kind, p).await?;
    Ok(Some(key(p)))
}

/// Where a page is kept in a generation's cache: the page set's own layout.
fn key(p: LevelPage) -> String {
    format!("{}/{}", p.level, p.page)
}

fn fault(data: Arc<Dataset>, runner: Arc<Runner>, kind: Kind, p: LevelPage) -> Pin<Box<dyn Future<Output = Result<(), Failure>> + Send>> {
    Box::pin(async move {
        let d = data.clone();
        data.cache
            .once(key(p), move |deadline| async move {
                if !d.source.holds(p) {
                    return d.cache.publish(&key(p), None).map_err(Failure::from);
                }
                if p.level > 0 {
                    for below in d.source.sources(p) {
                        tokio::time::timeout_at(deadline, fault(d.clone(), runner.clone(), kind, below))
                            .await
                            .map_err(|_| Failure::Limited("demand dependency deadline"))??;
                    }
                    if let Some((name, bytes)) = d.source.absent() {
                        d.cache.publish(name, Some(bytes))?;
                    }
                }
                let job = Job::Page {
                    kind,
                    subject: p.subject(),
                    cache: d.cache.root.clone(),
                };
                runner.run(job, d.clone(), deadline, Publication::Packet(key(p))).await
            })
            .await
    })
}

/// A page made in the job's own process: a leaf from the layer's source, a
/// page above it folded from the cache the leaves were published into.
pub(super) fn execute<L: Layer>(p: LevelPage, cache: &Path, out: &Path, leaf: impl FnOnce() -> Result<Option<L>, String>, absent: impl FnOnce() -> Result<L, String>) -> Result<(), String> {
    let page = if p.level == 0 { leaf()? } else { fold_stored(&PageSet::new(cache), p, &absent()?)? };
    let packet = page.map(|page| pack(&page.encode()?, Level::Live).map_err(|e| e.to_string())).transpose()?;
    job::write_packet(out, packet.as_deref())
}
