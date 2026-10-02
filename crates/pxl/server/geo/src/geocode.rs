// pxl-geo/geocode.rs

//! Which divisions contain a point, broad to fine, resolved from coordinates.

use std::collections::HashMap;

use deadpool_postgres::Pool;
use pxl_geo_spec::overture::division::{broad_rank, rank_of, FINE_SUBTYPE_PRIORITY};
use pxl_spec::place::PlacePart;
use tokio_postgres::Row;

use pxl_server::db::DbError;

fn names_of(common: &Option<serde_json::Value>) -> std::collections::BTreeMap<String, String> {
    let Some(serde_json::Value::Object(map)) = common else { return Default::default() };
    map.iter().filter_map(|(lang, name)| name.as_str().map(|n| (lang.clone(), n.to_string()))).collect()
}

/// `gers_id <> $3`: a division's own polygon contains its centroid and would
/// otherwise come back as its own parent.
const CONTAINING_BROAD_SQL: &str = "SELECT subtype, name, names_common
FROM overture_divisions
WHERE subtype IN ('region', 'county') AND gers_id <> $3
  AND ST_Contains(geom, ST_SetSRID(ST_MakePoint($1, $2), 3857))";

/// The batched form, one statement for a whole result page. A point with no
/// fine polygon gets the coarser label; there is no nearest-point fallback.
const CONTAINING_DIVISIONS_BATCH_SQL: &str = "SELECT p.idx, d.subtype, d.name, d.names_common
FROM unnest($1::float8[], $2::float8[]) WITH ORDINALITY AS p(x, y, idx)
JOIN overture_divisions d
  ON d.subtype IN ('region', 'county', 'locality', 'neighborhood', 'microhood', 'macrohood')
 AND ST_Contains(d.geom, ST_SetSRID(ST_MakePoint(p.x, p.y), 3857))";

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

    fn into_part(self) -> Option<PlacePart> {
        rank_of(&self.subtype).map(|rank| PlacePart::new(rank, self.name, names_of(&self.common)))
    }
}

/// Broad before narrow, each name once.
fn order_broad(parts: Vec<Part>) -> Vec<PlacePart> {
    let mut ordered: Vec<Part> = parts.into_iter().filter(|p| broad_rank(&p.subtype).is_some()).collect();
    ordered.sort_by_key(|p| broad_rank(&p.subtype));

    let mut seen: Vec<String> = Vec::new();
    let mut out: Vec<PlacePart> = Vec::new();
    for p in ordered {
        if seen.contains(&p.name) {
            continue;
        }
        seen.push(p.name.clone());
        out.extend(p.into_part());
    }
    out
}

/// `cx`/`cy` are EPSG:3857 metres, the units the division geometry is stored in.
pub async fn resolve_broad_context(pool: &Pool, cx: f64, cy: f64, exclude_gers_id: &str) -> Result<Vec<PlacePart>, DbError> {
    let client = pool.get().await?;
    let rows = client.query(CONTAINING_BROAD_SQL, &[&cx, &cy, &exclude_gers_id]).await?;
    Ok(order_broad(rows.iter().map(Part::from_row).collect()))
}

pub async fn resolve_point_context_batch(pool: &Pool, xs: &[f64], ys: &[f64]) -> Result<Vec<Vec<PlacePart>>, DbError> {
    let mut out: Vec<Vec<PlacePart>> = (0..xs.len()).map(|_| Vec::new()).collect();
    if xs.is_empty() {
        return Ok(out);
    }

    let client = pool.get().await?;
    let rows = client.query(CONTAINING_DIVISIONS_BATCH_SQL, &[&xs, &ys]).await?;

    let mut by_point: HashMap<usize, Vec<Part>> = HashMap::new();
    for row in &rows {
        let idx: i64 = row.get("idx");
        by_point.entry((idx - 1) as usize).or_default().push(Part::from_row(row));
    }

    for (i, list) in by_point {
        // The fine subtypes are alternatives, not a chain: the most specific wins.
        let fine = FINE_SUBTYPE_PRIORITY
            .iter()
            .find_map(|subtype| list.iter().position(|p| p.subtype == *subtype))
            .and_then(|at| rank_of(&list[at].subtype).map(|rank| PlacePart::new(rank, list[at].name.clone(), names_of(&list[at].common))));

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
