// pxl-geo/search/overture.rs

//! Forward search over `overture_places` (POI) and the division tables.

use deadpool_postgres::Pool;
use serde::Serialize;
use tokio_postgres::types::ToSql;
use tokio_postgres::Row;

use pxl_spec::place::PlacePart;

use crate::geocode;
use crate::search::endpoint::Near;
use pxl_server::db::DbError;

const DIVISION_LIMIT: i64 = 5;

/// ILIKE is the filter the trigram index accelerates; `%` alone rejects short
/// Korean queries like "강남", so `similarity()` only ranks.
macro_rules! places_matching {
    () => {
        "SELECT gers_id, name, properties,
  ST_X(geom) AS x3857, ST_Y(geom) AS y3857,
  ST_X(ST_Transform(geom, 4326)) AS lon, ST_Y(ST_Transform(geom, 4326)) AS lat
FROM overture_places WHERE name <> '' AND name ILIKE '%' || $1 || '%'
"
    };
}

const SEARCH_SQL: &str = concat!(places_matching!(), "ORDER BY similarity(name, $1) DESC, name\nLIMIT $2");

/// Ranked against `near`. Similarity is rounded to two decimals so distance
/// decides among near-equal names; on the raw score, one trigram outranked a
/// thousand kilometres.
const SEARCH_NEAR_SQL: &str = concat!(
    places_matching!(),
    "ORDER BY round(similarity(name, $1)::numeric, 2) DESC,
  geom <-> ST_Transform(ST_SetSRID(ST_MakePoint($3, $4), 4326), 3857) ASC
LIMIT $2"
);

/// A division is a row of `overture_divisions`, once, whatever areas it has.
const DIVISION_SEARCH_SQL: &str = concat!(
    "SELECT gers_id, name, subtype, ",
    pxl_geo_spec::division_names!("overture_divisions"),
    " AS names_common,
  ST_X(ST_Transform(geom, 4326)) AS lon, ST_Y(ST_Transform(geom, 4326)) AS lat
FROM overture_divisions
WHERE name <> '' AND name ILIKE '%' || $1 || '%'
ORDER BY similarity(name, $1) DESC, gers_id
LIMIT $2"
);

#[derive(Serialize)]
pub struct SearchResult {
    pub gers_id: String,
    pub kind: &'static str,
    pub name: String,
    pub category: String,
    /// Null for a POI: Places' `names.common` is ~0% populated for Korean.
    pub common: Option<serde_json::Value>,
    pub context: Vec<PlacePart>,
    pub lon: f64,
    pub lat: f64,
}

struct PlaceRow {
    gers_id: String,
    name: String,
    properties: serde_json::Value,
    x3857: f64,
    y3857: f64,
    lon: f64,
    lat: f64,
}

struct DivisionRow {
    gers_id: String,
    name: String,
    subtype: String,
    names_common: Option<serde_json::Value>,
    x3857: f64,
    y3857: f64,
    lon: f64,
    lat: f64,
}

impl PlaceRow {
    fn from_row(row: &Row) -> PlaceRow {
        PlaceRow {
            gers_id: row.get("gers_id"),
            name: row.get("name"),
            properties: row.get("properties"),
            x3857: row.get("x3857"),
            y3857: row.get("y3857"),
            lon: row.get("lon"),
            lat: row.get("lat"),
        }
    }
}

impl DivisionRow {
    fn from_row(row: &Row) -> DivisionRow {
        DivisionRow {
            gers_id: row.get("gers_id"),
            name: row.get("name"),
            subtype: row.get("subtype"),
            names_common: row.get("names_common"),
            x3857: row.get("x3857"),
            y3857: row.get("y3857"),
            lon: row.get("lon"),
            lat: row.get("lat"),
        }
    }
}

fn dedupe_divisions_by_name(rows: Vec<DivisionRow>) -> Vec<DivisionRow> {
    let mut seen: Vec<String> = Vec::new();
    let mut out: Vec<DivisionRow> = Vec::new();
    for r in rows {
        if seen.contains(&r.name) {
            continue;
        }
        seen.push(r.name.clone());
        out.push(r);
    }
    out
}

pub async fn search(pool: &Pool, query: &str, near: Option<&Near>, limit: u32) -> Result<Vec<SearchResult>, DbError> {
    let limit_i64 = limit as i64;

    // Survives the dedupe below dropping a name matched as both area and point.
    let division_fetch = DIVISION_LIMIT * 2;

    let (places_sql, places_params): (&str, Vec<&(dyn ToSql + Sync)>) = match near {
        Some(near) => (SEARCH_NEAR_SQL, vec![&query, &limit_i64, &near.lon, &near.lat]),
        None => (SEARCH_SQL, vec![&query, &limit_i64]),
    };
    let division_params: Vec<&(dyn ToSql + Sync)> = vec![&query, &division_fetch];

    let client = pool.get().await?;
    // One checkout: tokio-postgres pipelines both statements on it.
    let (places, divisions) = tokio::join!(client.query(places_sql, &places_params), client.query(DIVISION_SEARCH_SQL, &division_params),);

    let place_rows: Vec<PlaceRow> = places?.iter().map(PlaceRow::from_row).collect();
    let division_rows: Vec<DivisionRow> = dedupe_divisions_by_name(divisions?.iter().map(DivisionRow::from_row).collect())
        .into_iter()
        .take(DIVISION_LIMIT as usize)
        .collect();
    drop(client);

    let mut division_results: Vec<SearchResult> = Vec::with_capacity(division_rows.len());
    for r in division_rows {
        let context = geocode::resolve_broad_context(pool, r.x3857, r.y3857, &r.gers_id).await?;
        division_results.push(SearchResult {
            gers_id: r.gers_id,
            kind: "division",
            name: r.name,
            category: r.subtype,
            common: r.names_common,
            context,
            lon: r.lon,
            lat: r.lat,
        });
    }

    // Context comes from the point, not `addresses[0]`: a 강남역 row addressed
    // "Seoul" sits in 용인, and the field's format varies by contributor.
    let xs: Vec<f64> = place_rows.iter().map(|r| r.x3857).collect();
    let ys: Vec<f64> = place_rows.iter().map(|r| r.y3857).collect();
    let poi_context = geocode::resolve_point_context_batch(pool, &xs, &ys).await?;

    let poi_results = place_rows.into_iter().zip(poi_context).map(|(r, context)| SearchResult {
        gers_id: r.gers_id,
        name: r.name,
        kind: "poi",
        category: pxl_geo_spec::overture::properties::category_of(&r.properties),
        common: None,
        context,
        lon: r.lon,
        lat: r.lat,
    });

    Ok(division_results.into_iter().chain(poi_results).take(limit as usize).collect())
}
