// pxl-geo-bake/demand/pyramid.rs

//! A pyramid layer baked a page at a time on demand. Lower pages are faulted
//! in one at a time, so a coarse request cannot fan out a task tree.

use super::job::{self, Job, Publication, Runner};
use super::{Dataset, Failure, Kind};
use crate::pyramid::{fold_stored, Absent, Layer, PageSet};
use crate::util::{pack, Level};
use pxl_spec::bake::quadkey::page_of_name;
use pxl_spec::space::pyramid::{Page, GROUND_Z};
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;

pub(super) async fn resolve(data: Arc<Dataset>, runner: Arc<Runner>, kind: Kind, name: &str, top: u32) -> Result<Option<String>, Failure> {
    let p = page_of_name(name).ok_or(Failure::BadName)?;
    if p.z < top {
        return Err(Failure::Unavailable);
    }
    fault(data, runner, kind, p).await?;
    Ok(Some(key(p)))
}

fn key(p: Page) -> String {
    format!("z{}/{}", p.z, p.page)
}

fn fault(data: Arc<Dataset>, runner: Arc<Runner>, kind: Kind, p: Page) -> Pin<Box<dyn Future<Output = Result<(), Failure>> + Send>> {
    Box::pin(async move {
        let d = data.clone();
        data.cache
            .once(key(p), move |deadline| async move {
                if !d.source.holds(p) {
                    return d.cache.publish(&key(p), None).map_err(Failure::from);
                }
                if p.z < GROUND_Z {
                    for below in d.source.sources(p) {
                        tokio::time::timeout_at(deadline, fault(d.clone(), runner.clone(), kind, below))
                            .await
                            .map_err(|_| Failure::Limited("demand dependency deadline"))??;
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

pub(super) fn execute<L: Layer>(p: Page, cache: &Path, out: &Path, leaf: impl FnOnce() -> Result<Option<L>, String>, absent: &impl Absent<L>) -> Result<(), String> {
    let page = if p.z == GROUND_Z { leaf()? } else { fold_stored(&PageSet::new(cache), p, absent)? };
    let packet = page.map(|page| pack(&page.encode()?, Level::Live).map_err(|e| e.to_string())).transpose()?;
    job::write_packet(out, packet.as_deref())
}
