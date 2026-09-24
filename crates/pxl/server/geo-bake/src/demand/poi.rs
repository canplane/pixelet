// pxl-geo-bake/demand/poi.rs

use super::job::{self, Job, Publication, Runner};
use super::{Dataset, Failure};
use pxl_spec::poi::xy;
use std::sync::Arc;

pub(super) async fn resolve(data: Arc<Dataset>, runner: Arc<Runner>, name: &str) -> Result<String, Failure> {
    let key = xy::parse(name)
        .or_else(|| pxl_spec::poi::detail::parse(name).map(|p| p.0))
        .filter(|k| xy::depth(*k).is_some_and(|d| d >= xy::ROOT_DEPTH))
        .ok_or(Failure::BadName)?;
    let root = key >> (2 * (xy::depth(key).unwrap() - xy::ROOT_DEPTH));
    let region = xy::name(root);
    let d = data.clone();
    let target = region.clone();
    data.cache
        .once(region.clone(), move |deadline| async move {
            runner
                .run(
                    Job::Poi { root, revision: d.revision },
                    d.clone(),
                    deadline,
                    Publication::Directory { source: "snapshot".into(), target },
                )
                .await
        })
        .await?;
    let path = format!("{region}/tile/{name}");
    // A completed snapshot is authoritative, including a request beyond a
    // terminal node. No missing file during construction is exposed as empty.
    if !data.cache.root.join(&path).is_file() {
        data.cache.publish(&path, None)?;
    }
    Ok(path)
}

pub(super) fn execute(root: u64, revision: u64, out: &std::path::Path, seconds: u64) -> Result<(), String> {
    use crate::poi::{provider::overture, snapshot};
    let mut client = job::connect(seconds)?;
    snapshot::write(&out.join("snapshot"), root, revision, "overture", crate::util::Level::Live, |accept| {
        overture::visit(&mut client, root, accept)
    })
}
