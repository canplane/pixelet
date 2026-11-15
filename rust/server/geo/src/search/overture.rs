// pxl-geo/search/overture.rs

//! Forward search over `overture_places` (places) and Overture's divisions.

use deadpool_postgres::Pool;
use serde::Serialize;
use tokio_postgres::Row;
use tokio_postgres::types::ToSql;

use pxl_spec::bake::tile::place::category_of;
use pxl_spec::division::{DivisionPart, Subtype};
use pxl_spec::search::search_kind;
use pxl_spec::space::coord::adapter::epsg4326;

use crate::division;
use crate::search::endpoint::Near;
use pxl_geo_spec::overture::properties;
use pxl_server::db::DbError;

const DIVISION_LIMIT: i64 = 5;

/// The places matching `$1`, numbered by `order`, the first `$2`. ILIKE is the
/// filter the trigram index accelerates; `%` alone rejects a query of two or
/// three characters, so `similarity()` only ranks. A query too short for a
/// trigram walks the index on `name`, which holds what this reads, not the rows.
macro_rules! places_matching {
    ($order:expr) => {
        concat!(
            "hit AS (SELECT gers_id, row_number() OVER (ORDER BY ",
            $order,
            ") AS at FROM overture_places WHERE name <> '' AND name ILIKE '%' || $1 || '%' ORDER BY at LIMIT $2)
SELECT p.gers_id, p.name, p.properties,
  ST_X(p.geom) AS x3857, ST_Y(p.geom) AS y3857,
  ST_X(ST_Transform(p.geom, 4326)) AS lon, ST_Y(ST_Transform(p.geom, 4326)) AS lat
FROM hit JOIN overture_places p USING (gers_id) ORDER BY hit.at"
        )
    };
}

const SEARCH_SQL: &str = concat!(
    "WITH ",
    places_matching!("similarity(name, $1) DESC, name, gers_id")
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

/// Ranked against `near`: its country's places first, so another country's
/// fill only what room is left. Similarity is rounded to two decimals so
/// distance decides among near-equal names. `home` is materialized, or each
/// matching place reads the country's polygons again.
const SEARCH_NEAR_SQL: &str = concat!(
    "WITH home AS MATERIALIZED (SELECT geom FROM overture_division_areas WHERE properties->>'division_id' = ",
    home!(),
    "),\n",
    places_matching!(
        "EXISTS (SELECT 1 FROM home WHERE ST_Intersects(home.geom, overture_places.geom)) DESC,
  round(similarity(name, $1)::numeric, 2) DESC,
  geom <-> ST_Transform(ST_SetSRID(ST_MakePoint($3, $4), 4326), 3857) ASC, gers_id"
    )
);

/// A division is a row of `overture_divisions`, once, whatever areas it has.
/// Found by a name in any language (`names`), and answered in its own.
macro_rules! divisions_matching {
    () => {
        "SELECT gers_id, name, subtype,
  ST_X(ST_Transform(geom, 4326)) AS lon, ST_Y(ST_Transform(geom, 4326)) AS lat
FROM overture_divisions
WHERE name <> '' AND names ILIKE '%' || $1 || '%'
"
    };
}

const DIVISION_SEARCH_SQL: &str = concat!(
    divisions_matching!(),
    "ORDER BY word_similarity($1, names) DESC, similarity(name, $1) DESC, gers_id\nLIMIT $2"
);

/// `near`'s country, and the divisions whose lineage holds it, first.
const DIVISION_SEARCH_NEAR_SQL: &str = concat!(
    "WITH home AS (SELECT ",
    home!(),
    " AS id)\n",
    divisions_matching!(),
    "ORDER BY (SELECT gers_id = home.id OR properties->'hierarchies'->0 @> jsonb_build_array(jsonb_build_object('division_id', home.id)) FROM home) DESC NULLS LAST,
  word_similarity($1, names) DESC, similarity(name, $1) DESC, gers_id
LIMIT $2"
);

/// A search result, its kind and category by number (`spec/spec/search.json5`).
#[derive(Serialize)]
pub(crate) struct SearchResult {
    pub gers_id: String,
    /// A `search_kind` number.
    pub kind: u8,
    pub name: String,
    /// A place's category (`category_of`), a division's subtype.
    pub category: u16,
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
            lon: row.get("lon"),
            lat: row.get("lat"),
        }
    }
}

pub(crate) async fn search(
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
        // A subtype this tree does not know has no number, so the row is skipped, as the division
        // tiles skip it.
        let Some(subtype) = Subtype::of_word(&r.subtype) else {
            continue;
        };
        let context = division::ancestors(pool, &r.gers_id).await?;
        division_results.push(SearchResult {
            gers_id: r.gers_id,
            kind: search_kind::DIVISION,
            name: r.name,
            category: u16::from(subtype as u8),
            context,
            lon: r.lon,
            lat: r.lat,
            dist_m: dist_m(near, r.lon, r.lat),
        });
    }

    // Context comes from the point, not `addresses[0]`: a row can be addressed
    // to the city it is named for while it sits in the next.
    let xs: Vec<f64> = place_rows.iter().map(|r| r.x3857).collect();
    let ys: Vec<f64> = place_rows.iter().map(|r| r.y3857).collect();
    let place_ctx = division::name_points(pool, &xs, &ys).await?;

    let place_results = place_rows
        .into_iter()
        .zip(place_ctx)
        .map(|(r, context)| SearchResult {
            gers_id: r.gers_id,
            name: r.name,
            kind: search_kind::PLACE,
            category: category_of(&properties::category_of(&r.properties)),
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
