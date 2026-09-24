// pxl-geo-bake/demand/poi.rs

use super::{Dataset, Failure, Pool};
use crate::poi::{provider::overture, snapshot};
use crate::util::Level;
use pxl_spec::poi::xy;
use std::sync::Arc;

pub(super) async fn resolve(data: Arc<Dataset>, pool: Arc<Pool>, name: &str) -> Result<String, Failure> {
    let key = xy::parse(name).filter(|k| xy::depth(*k).is_some_and(|d| d >= xy::ROOT_DEPTH)).ok_or(Failure::BadName)?;
    let root = key >> (2 * (xy::depth(key).unwrap() - xy::ROOT_DEPTH));
    let region = xy::name(root);
    let d = data.clone();
    let dir = data.cache.root.join(&region);
    data.cache
        .once(region.clone(), async move {
            tokio::task::spawn_blocking(move || pool.with(|w| snapshot::write(&dir, root, d.revision, "overture", Level::Live, |accept| overture::visit(&mut w.client, root, accept))))
                .await
                .map_err(|e| e.to_string())?
        })
        .await?;
    let path = format!("{region}/tile/{name}");
    // A completed snapshot is authoritative, including a request beyond a
    // terminal node. No missing file during construction is exposed as empty.
    let d = data.clone();
    let output = path.clone();
    data.cache
        .once(path.clone(), async move {
            if !d.cache.root.join(&output).is_file() {
                d.cache.publish(&output, None)?;
            }
            Ok(())
        })
        .await?;
    Ok(path)
}
