// pxl-geo/search/overture.rs

//! Forward search over `overture_places` (places) and Overture's divisions.

use deadpool_postgres::Pool;
use serde::Serialize;
use tokio_postgres::types::ToSql;
use tokio_postgres::Row;

use pxl_spec::division::DivisionPart;
use pxl_spec::space::coord::adapter::epsg4326;

use crate::division;
use crate::search::endpoint::Near;
use pxl_server::db::DbError;

const DIVISION_LIMIT: i64 = 5;

/// ILIKE is the filter the trigram index accelerates; `%` alone rejects a
/// query of two or three characters, so `similarity()` only ranks.
macro_rules! places_matching {
    () => {
        "SELECT gers_id, name, properties,
  ST_X(geom) AS x3857, ST_Y(geom) AS y3857,
  ST_X(ST_Transform(geom, 4326)) AS lon, ST_Y(ST_Transform(geom, 4326)) AS lat
FROM overture_places WHERE name <> '' AND name ILIKE '%' || $1 || '%'
"
    };
}

const SEARCH_SQL: &str = concat!(
    places_matching!(),
    "ORDER BY similarity(name, $1) DESC, name, gers_id\nLIMIT $2"
);

/// The country division `near` (`$3`, `$4`) lies in; null at sea or past the
/// divisions.
macro_rules! home {
    () => {
        "(SELECT s.properties->>'division_id' FROM overture_division_areas s
  WHERE s.subtype = 'country' AND ST_Contains(s.geom, ST_Transform(ST_SetSRID(ST_MakePoint($3, $4), 4326), 3857))
  ORDER BY s.gers_id LIMIT 1)"
    };
}

/// Ranked against `near`: its country's places first, as Google's Places
/// API biases suggestions to the request's `regionCode`, so another country's
/// fill only what room is left. Similarity is rounded to two decimals so
/// distance decides among near-equal names; on the raw score, one trigram
/// outranked a thousand kilometers.
const SEARCH_NEAR_SQL: &str = concat!(
    "WITH home AS (SELECT geom FROM overture_division_areas WHERE properties->>'division_id' = ",
    home!(),
    ")\n",
    places_matching!(),
    "ORDER BY EXISTS (SELECT 1 FROM home WHERE ST_Intersects(home.geom, overture_places.geom)) DESC,
  round(similarity(name, $1)::numeric, 2) DESC,
  geom <-> ST_Transform(ST_SetSRID(ST_MakePoint($3, $4), 4326), 3857) ASC, gers_id
LIMIT $2"
);

/// A division is a row of `overture_divisions`, once, whatever areas it has.
macro_rules! divisions_matching {
    () => {
        concat!(
            "SELECT gers_id, name, subtype, ",
            pxl_geo_spec::division_names!("overture_divisions"),
            " AS names_common,
  ST_X(ST_Transform(geom, 4326)) AS lon, ST_Y(ST_Transform(geom, 4326)) AS lat
FROM overture_divisions
WHERE name <> '' AND name ILIKE '%' || $1 || '%'
"
        )
    };
}

const DIVISION_SEARCH_SQL: &str = concat!(
    divisions_matching!(),
    "ORDER BY similarity(name, $1) DESC, gers_id\nLIMIT $2"
);

/// `near`'s country, and the divisions its lineage holds, first.
const DIVISION_SEARCH_NEAR_SQL: &str = concat!(
    "WITH home AS (SELECT ",
    home!(),
    " AS id)\n",
    divisions_matching!(),
    "ORDER BY (SELECT gers_id = home.id OR properties->'hierarchies'->0 @> jsonb_build_array(jsonb_build_object('division_id', home.id)) FROM home) DESC NULLS LAST,
  similarity(name, $1) DESC, gers_id
LIMIT $2"
);

#[derive(Serialize)]
pub struct SearchResult {
    pub gers_id: String,
    pub kind: &'static str,
    pub name: String,
    pub category: String,
    /// Null for a places: Places' `names.common` is ~0% populated for Korean.
    pub common: Option<serde_json::Value>,
    pub context: Vec<DivisionPart>,
    pub lon: f64,
    pub lat: f64,
    /// The ground meters from `near` (`epsg4326::dist`); null without one.
    pub dist_m: Option<f64>,
}

/// The ground meters from `near` to `lon`, `lat`.
fn dist_m(near: Option<&Near>, lon: f64, lat: f64) -> Option<f64> {
    near.map(|n| epsg4326::dist(&[n.lon, n.lat, 0.0], &[lon, lat, 0.0]))
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
            lon: row.get("lon"),
            lat: row.get("lat"),
        }
    }
}

pub async fn search(
    pool: &Pool,
    query: &str,
    near: Option<&Near>,
    limit: u32,
) -> Result<Vec<SearchResult>, DbError> {
    let limit_i64 = limit as i64;

    let (places_sql, places_params): (&str, Vec<&(dyn ToSql + Sync)>) = match near {
        Some(near) => (
            SEARCH_NEAR_SQL,
            vec![&query, &limit_i64, &near.lon, &near.lat],
        ),
        None => (SEARCH_SQL, vec![&query, &limit_i64]),
    };
    let (division_sql, division_params): (&str, Vec<&(dyn ToSql + Sync)>) = match near {
        Some(near) => (
            DIVISION_SEARCH_NEAR_SQL,
            vec![&query, &DIVISION_LIMIT, &near.lon, &near.lat],
        ),
        None => (DIVISION_SEARCH_SQL, vec![&query, &DIVISION_LIMIT]),
    };

    let client = pool.get().await?;
    // One checkout: tokio-postgres pipelines both statements on it.
    let (places, divisions) = tokio::join!(
        client.query(places_sql, &places_params),
        client.query(division_sql, &division_params),
    );

    let place_rows: Vec<PlaceRow> = places?.iter().map(PlaceRow::from_row).collect();
    let division_rows: Vec<DivisionRow> = divisions?.iter().map(DivisionRow::from_row).collect();
    drop(client);

    let mut division_results: Vec<SearchResult> = Vec::with_capacity(division_rows.len());
    for r in division_rows {
        let context = division::ancestors(pool, &r.gers_id).await?;
        division_results.push(SearchResult {
            gers_id: r.gers_id,
            kind: "division",
            name: r.name,
            category: r.subtype,
            common: r.names_common,
            context,
            lon: r.lon,
            lat: r.lat,
            dist_m: dist_m(near, r.lon, r.lat),
        });
    }

    // Context comes from the point, not `addresses[0]`: a row can be addressed
    // to the city it is named for while it sits in the next, and the field's
    // format varies by contributor.
    let xs: Vec<f64> = place_rows.iter().map(|r| r.x3857).collect();
    let ys: Vec<f64> = place_rows.iter().map(|r| r.y3857).collect();
    let place_context = division::name_points(pool, &xs, &ys).await?;

    let place_results = place_rows
        .into_iter()
        .zip(place_context)
        .map(|(r, context)| SearchResult {
            gers_id: r.gers_id,
            name: r.name,
            kind: "place",
            category: pxl_geo_spec::overture::properties::category_of(&r.properties),
            common: None,
            context,
            lon: r.lon,
            lat: r.lat,
            dist_m: dist_m(near, r.lon, r.lat),
        });

    Ok(division_results
        .into_iter()
        .chain(place_results)
        .take(limit as usize)
        .collect())
}
