// pxl-geo/division.rs

//! Which divisions name a point, broad to fine, resolved from coordinates. A
//! point steps down by the divisions' rule (pxl-geo-spec `overture::division`),
//! among the children whose points lie within `BAKE_READ_DIST` of it on either
//! axis. Where no area holds the point, it is in the nearest division within
//! that reach, approximately.

/// Each point's deepest division whose area holds it, the lesser id at one
/// depth, and where none does, the nearest division within `$3` on either
/// axis: `$1`/`$2` are EPSG:3857 meters and `idx` counts the points from 1.
const AT_SQL: &str = "
SELECT p.idx, a.id AS area, n.id AS nearest
FROM unnest($1::float8[], $2::float8[]) WITH ORDINALITY AS p(x, y, idx)
CROSS JOIN LATERAL (SELECT ST_SetSRID(ST_MakePoint(p.x, p.y), 3857) AS pt) g
LEFT JOIN LATERAL (
  SELECT d.gers_id AS id FROM overture_division_areas s JOIN overture_divisions d ON d.gers_id = s.properties->>'division_id'
  WHERE ST_Contains(s.geom, g.pt)
  ORDER BY jsonb_array_length(d.properties->'hierarchies'->0) DESC, d.gers_id LIMIT 1
) a ON true
LEFT JOIN LATERAL (
  SELECT q.gers_id AS id FROM overture_divisions q
  WHERE a.id IS NULL AND q.geom && ST_MakeEnvelope(p.x - $3, p.y - $3, p.x + $3, p.y + $3, 3857)
  ORDER BY q.geom <-> g.pt, q.gers_id LIMIT 1
) n ON true
WHERE a.id IS NOT NULL OR n.id IS NOT NULL";

/// The divisions with no area whose point lies within `$3` of each point, with
/// their parents, as `AT_SQL` takes the points.
const CHILDREN_SQL: &str = "
SELECT p.idx, d.gers_id, d.properties->>'parent_division_id' AS parent, ST_X(d.geom) AS x, ST_Y(d.geom) AS y
FROM unnest($1::float8[], $2::float8[]) WITH ORDINALITY AS p(x, y, idx)
JOIN overture_divisions d ON d.geom && ST_MakeEnvelope(p.x - $3, p.y - $3, p.x + $3, p.y + $3, 3857)
WHERE NOT EXISTS (SELECT 1 FROM overture_division_areas s WHERE s.properties->>'division_id' = d.gers_id)";

use fnv::FnvHashMap;

use deadpool_postgres::Pool;
use pxl_geo_spec::BAKE_READ_DIST;
use pxl_geo_spec::overture::division::{self, Children, Division, LINEAGE_SQL, steps};
use pxl_spec::division::DivisionPart;
use pxl_spec::space::coord::adapter::epsg3857::merc_from_u;
use tokio_postgres::Client;

use pxl_server::db::DbError;

/// The divisions `ids` names and every ancestor of each, by id.
async fn lineage(client: &Client, ids: &[String]) -> Result<FnvHashMap<String, Division>, DbError> {
    let rows = client.query(LINEAGE_SQL, &[&ids]).await?;
    Ok(rows
        .iter()
        .map(|r| {
            Division::new(
                r.get("gers_id"),
                r.get("subtype"),
                r.get::<_, Option<String>>("name").unwrap_or_default(),
                r.get("parent"),
            )
        })
        .map(|d| (d.gers_id.clone(), d))
        .collect())
}

/// What a division lies in, broad to narrow: its name less itself.
pub(crate) async fn ancestors(pool: &Pool, id: &str) -> Result<Vec<DivisionPart>, DbError> {
    let client = pool.get().await?;
    let divisions = lineage(&client, &[id.to_string()]).await?;
    let mut parts = division::name(id, false, &divisions);
    parts.pop();
    Ok(parts)
}

/// Each point's name; `xs`/`ys` are EPSG:3857 meters.
pub(crate) async fn name_points(
    pool: &Pool,
    xs: &[f64],
    ys: &[f64],
) -> Result<Vec<Vec<DivisionPart>>, DbError> {
    if xs.is_empty() {
        return Ok(Vec::new());
    }
    let client = pool.get().await?;
    let reach = merc_from_u(BAKE_READ_DIST);
    let mut children = vec![Children::default(); xs.len()];
    for r in client.query(CHILDREN_SQL, &[&xs, &ys, &reach]).await? {
        if let Some(parent) = r.get::<_, Option<String>>("parent") {
            children[(r.get::<_, i64>("idx") - 1) as usize]
                .entry(parent)
                .or_default()
                .push(([r.get("x"), r.get("y")], r.get("gers_id")));
        }
    }
    let mut finest: Vec<Option<(String, bool)>> = vec![None; xs.len()];
    for r in client.query(AT_SQL, &[&xs, &ys, &reach]).await? {
        let k = (r.get::<_, i64>("idx") - 1) as usize;
        finest[k] = match r.get::<_, Option<String>>("area") {
            Some(area) => match steps(&area, [xs[k], ys[k]], &children[k]).last() {
                Some(id) => Some((id.to_string(), true)),
                None => Some((area, false)),
            },
            None => r.get::<_, Option<String>>("nearest").map(|id| (id, true)),
        };
    }
    let ids: Vec<String> = finest.iter().flatten().map(|(id, _)| id.clone()).collect();
    let divisions = lineage(&client, &ids).await?;
    Ok(finest
        .iter()
        .map(|f| {
            f.as_ref().map_or_else(Vec::new, |(id, approx)| {
                division::name(id, *approx, &divisions)
            })
        })
        .collect())
}
