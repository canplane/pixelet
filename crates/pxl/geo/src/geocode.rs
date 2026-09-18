// pxl-geo/geocode.rs

//! Which divisions contain a point, broad to fine.
//!
//! A division result like 수내동 is easy to misread without knowing which
//! 성남시 or 분당구 it sits in; a POI result named 강남역 is worse, because
//! there is more than one and their names are identical. This answers "which
//! of these is this one" for both kinds, and it answers it from the
//! coordinates rather than from any text on the row.

use std::collections::HashMap;

use deadpool_postgres::Pool;
pub use pxl_overture::division::GeocodePart;
use pxl_overture::division::{broad_rank, FINE_SUBTYPE_PRIORITY};
use tokio_postgres::Row;

use crate::db::DbError;

/// `gers_id <> $3` matters when the point being resolved is itself a
/// region/county centroid (e.g. resolving context for a "성남시" match) —
/// without it, a county's own polygon (which contains its own centroid) would
/// show up as its own parent.
const CONTAINING_BROAD_SQL: &str = "SELECT subtype, name, names_common
FROM overture_divisions
WHERE subtype IN ('region', 'county') AND gers_id <> $3
  AND ST_Contains(geom, ST_SetSRID(ST_MakePoint($1, $2), 3857))";

/// The same question for many points at once, and down to the fine level.
///
/// One statement for the whole set: search resolves context for a result
/// page, and twenty round trips to answer twenty containments is twenty times
/// the latency for the same index scan. `overture_divisions.geom` carries a
/// GiST index, so each point is a lookup rather than a scan.
///
/// The subtype list is the answer's own shape rather than a tuning choice:
/// these six are what the ordering below can use, so joining the rest fetches
/// rows to discard. `country` is the one worth naming — its polygon is the
/// whole of Korea, and an exact containment against it is not cheap.
///
/// What this does not do is the reverse geocode's nearest-point fallback: a
/// point with no containing fine polygon gets the coarser label instead of a
/// second query, which for telling two same-named search results apart is
/// already enough.
const CONTAINING_DIVISIONS_BATCH_SQL: &str = "SELECT p.idx, d.subtype, d.name, d.names_common
FROM unnest($1::float8[], $2::float8[]) WITH ORDINALITY AS p(x, y, idx)
JOIN overture_divisions d
  ON d.subtype IN ('region', 'county', 'locality', 'neighborhood', 'microhood', 'macrohood')
 AND ST_Contains(d.geom, ST_SetSRID(ST_MakePoint(p.x, p.y), 3857))";

/// One matched division row, before it is ordered into parts.
struct Part {
    subtype: String,
    name: String,
    common: Option<serde_json::Value>,
}

impl Part {
    fn from_row(row: &Row) -> Part {
        Part {
            subtype: row.get("subtype"),
            name: row.get("name"),
            common: row.get("names_common"),
        }
    }

    fn into_geocode_part(self) -> GeocodePart {
        GeocodePart::new(self.name, self.common)
    }
}

/// Broad before narrow, each name once. Shared by both resolvers so a batched
/// answer and a single one cannot order themselves differently.
fn order_broad(parts: Vec<Part>) -> Vec<GeocodePart> {
    let mut ordered: Vec<Part> = parts.into_iter().filter(|p| broad_rank(&p.subtype).is_some()).collect();
    ordered.sort_by_key(|p| broad_rank(&p.subtype));

    let mut seen: Vec<String> = Vec::new();
    let mut out: Vec<GeocodePart> = Vec::new();
    for p in ordered {
        if seen.contains(&p.name) {
            continue;
        }
        seen.push(p.name.clone());
        out.push(p.into_geocode_part());
    }
    out
}

/// `cx`/`cy` in EPSG:3857 meters (the units `overture_divisions.geom` and
/// `overture_division_points.geom` are stored in), not WGS84 lon/lat.
/// `exclude_gers_id` is the division being resolved itself, so it never shows
/// up as its own parent.
pub async fn resolve_broad_context(pool: &Pool, cx: f64, cy: f64, exclude_gers_id: &str) -> Result<Vec<GeocodePart>, DbError> {
    let client = pool.get().await?;
    let rows = client.query(CONTAINING_BROAD_SQL, &[&cx, &cy, &exclude_gers_id]).await?;
    Ok(order_broad(rows.iter().map(Part::from_row).collect()))
}

pub async fn resolve_point_context_batch(pool: &Pool, xs: &[f64], ys: &[f64]) -> Result<Vec<Vec<GeocodePart>>, DbError> {
    let mut out: Vec<Vec<GeocodePart>> = (0..xs.len()).map(|_| Vec::new()).collect();
    if xs.is_empty() {
        return Ok(out);
    }

    let client = pool.get().await?;
    let rows = client.query(CONTAINING_DIVISIONS_BATCH_SQL, &[&xs, &ys]).await?;

    let mut by_point: HashMap<usize, Vec<Part>> = HashMap::new();
    for row in &rows {
        // WITH ORDINALITY counts from one.
        let idx: i64 = row.get("idx");
        by_point.entry((idx - 1) as usize).or_default().push(Part::from_row(row));
    }

    for (i, list) in by_point {
        // The fine subtypes are alternatives rather than a chain, so the most
        // specific one that matched wins and the rest are dropped.
        let fine = FINE_SUBTYPE_PRIORITY
            .iter()
            .find_map(|subtype| list.iter().position(|p| p.subtype == *subtype))
            .map(|at| GeocodePart::new(list[at].name.clone(), list[at].common.clone()));

        let mut parts = order_broad(list);
        if let Some(fine) = fine {
            if !parts.iter().any(|p| p.name == fine.name) {
                parts.push(fine);
            }
        }
        out[i] = parts;
    }

    Ok(out)
}
