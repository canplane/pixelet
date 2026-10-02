// pxl-geo/geocode.rs

//! Which divisions name a point, broad to fine, resolved from coordinates.

use deadpool_postgres::Pool;
use pxl_geo_spec::overture::division::{self, Division, CONTAINING_SQL, NEAREST_POINT_SQL};
use pxl_spec::place::PlacePart;
use tokio_postgres::Row;

use pxl_server::db::DbError;

fn division_of(row: &Row) -> (i64, Division) {
    (row.get("idx"), Division::new(row.get("gers_id"), row.get("subtype"), row.get("name"), row.get("names_common")))
}

/// The broad areas around a division; `cx`/`cy` are EPSG:3857 metres. The
/// division's own polygon contains its centroid and is not its own parent.
pub async fn resolve_broad_context(pool: &Pool, cx: f64, cy: f64, exclude_gers_id: &str) -> Result<Vec<PlacePart>, DbError> {
    let client = pool.get().await?;
    let rows = client.query(CONTAINING_SQL, &[&[cx].as_slice(), &[cy].as_slice()]).await?;
    let areas: Vec<Division> = rows.iter().map(division_of).map(|(_, d)| d).filter(|d| d.gers_id != exclude_gers_id).collect();
    Ok(division::broad(&areas).into_iter().map(|d| d.part(false)).collect())
}

pub async fn resolve_point_context_batch(pool: &Pool, xs: &[f64], ys: &[f64]) -> Result<Vec<Vec<PlacePart>>, DbError> {
    if xs.is_empty() {
        return Ok(Vec::new());
    }
    let client = pool.get().await?;
    let areas = division::by_point(xs.len(), client.query(CONTAINING_SQL, &[&xs, &ys]).await?.iter().map(division_of));
    let within: Vec<Option<String>> = areas.iter().map(|a| division::within(a)).collect();
    let points = division::by_point(xs.len(), client.query(NEAREST_POINT_SQL, &[&xs, &ys, &within]).await?.iter().map(division_of));
    Ok(areas
        .iter()
        .zip(&points)
        .map(|(areas, point)| division::label(areas, point.first()).into_iter().map(|(d, approx)| d.part(approx)).collect())
        .collect())
}
