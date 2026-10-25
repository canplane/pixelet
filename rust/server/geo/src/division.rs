// pxl-geo/division.rs

//! Which divisions name a point, broad to fine, resolved from coordinates.

use fnv::FnvHashMap;

use deadpool_postgres::Pool;
use pxl_geo_spec::overture::division::{self, Division, FINEST_SQL, LINEAGE_SQL};
use pxl_spec::division::DivisionPart;
use tokio_postgres::Client;

use pxl_server::db::DbError;

/// The divisions `ids` names and every ancestor of each, by id.
async fn lineage(client: &Client, ids: &[String]) -> Result<FnvHashMap<String, Division>, DbError> {
    let rows = client.query(LINEAGE_SQL, &[&ids]).await?;
    Ok(rows
        .iter()
        .map(|r| Division::new(r.get("gers_id"), r.get::<_, &str>("subtype"), r.get("name"), r.get("names_common"), r.get("parent")))
        .map(|d| (d.gers_id.clone(), d))
        .collect())
}

/// What a division lies in, broad to narrow: its name less itself.
pub async fn ancestors(pool: &Pool, id: &str) -> Result<Vec<DivisionPart>, DbError> {
    let client = pool.get().await?;
    let held = lineage(&client, &[id.to_string()]).await?;
    let mut parts = division::name(id, false, &held);
    parts.pop();
    Ok(parts)
}

/// Each point's name; `xs`/`ys` are EPSG:3857 metres.
pub async fn name_points(pool: &Pool, xs: &[f64], ys: &[f64]) -> Result<Vec<Vec<DivisionPart>>, DbError> {
    if xs.is_empty() {
        return Ok(Vec::new());
    }
    let client = pool.get().await?;
    let reach: Vec<f64> = ys.iter().map(|&y| division::reach_at(y)).collect();
    let rows = client.query(FINEST_SQL, &[&xs, &ys, &reach]).await?;
    let finest = division::by_point(xs.len(), rows.iter().map(|r| (r.get("idx"), r.get("division_id"), r.get("approx"))));
    let ids: Vec<String> = finest.iter().flatten().map(|(id, _)| id.clone()).collect();
    let held = lineage(&client, &ids).await?;
    Ok(finest.iter().map(|f| f.as_ref().map_or_else(Vec::new, |(id, approx)| division::name(id, *approx, &held))).collect())
}
