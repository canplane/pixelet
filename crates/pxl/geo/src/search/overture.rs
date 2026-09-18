// pxl-geo/search/overture.rs

//! Forward search over Overture's Places theme (`overture_places`, the same
//! table POI reads) for `kind: "poi"`, plus `overture_divisions` and
//! `overture_division_points` (the same tables reverse geocoding reads) for
//! `kind: "division"`.
//!
//! Both halves are needed: without the division half, a query like
//! "서울특별시" or "수내동" only ever matched a POI whose own name happened to
//! contain it — a hospital literally named "서울특별시 보라매병원", not the
//! city. A division's `lon`/`lat` is its polygon centroid (or the bare point,
//! when only `overture_division_points` has one), and jumping there behaves
//! exactly like a POI jump rather than zooming out to fit the whole area.

use deadpool_postgres::Pool;
use serde::Serialize;
use tokio_postgres::types::ToSql;
use tokio_postgres::Row;

use crate::db::DbError;
use crate::geocode::{self, GeocodePart};
use crate::search::Near;

/// Divisions never crowd out POI results — a handful of admin-area matches
/// (there is rarely more than one real "서울특별시") capped well under the
/// overall limit, with the rest of the budget going to POI.
const DIVISION_LIMIT: i64 = 5;

/// `name ILIKE '%'||$1||'%'` is what the GIN gin_trgm_ops index on
/// `overture_places.name` actually accelerates — trigram's `%` similarity
/// operator alone would reject very short Korean queries (a 2-syllable name
/// like "강남" barely forms enough trigrams to clear the default 0.3
/// similarity threshold), so ILIKE is the primary filter and `similarity()`
/// is only the ranking signal, not the WHERE clause.
///
/// Two separate statements rather than one with an optional param: ORDER BY
/// needs a genuinely different expression when a bias point exists, not just
/// a NULL-able argument to the same one.
const SEARCH_SQL: &str = "SELECT gers_id, name, properties,
  ST_X(geom) AS x3857, ST_Y(geom) AS y3857,
  ST_X(ST_Transform(geom, 4326)) AS lon, ST_Y(ST_Transform(geom, 4326)) AS lat
FROM overture_places
WHERE name <> '' AND name ILIKE '%' || $1 || '%'
ORDER BY similarity(name, $1) DESC, name
LIMIT $2";

/// Same query, ranked against `near` — the version that answers "which
/// 강남역" and "which 스타벅스", which is what a map search is usually being
/// asked.
///
/// Similarity is rounded to two decimals before it is ordered on, and that
/// rounding is the whole point. Ordering on the raw score made distance a
/// tiebreaker for exactly-equal names and nothing else, so "스타벅스 강남대로"
/// at 0.61 sorted ahead of every "스타벅스" at 0.60 no matter which continent
/// each was on — a difference of one trigram outranking a difference of a
/// thousand kilometres. A band collapses scores that are indistinguishable as
/// answers into one tie, and distance then decides inside the band. Two
/// decimals is roughly "the same name, modulo a word": it does not let a
/// genuinely worse match win on proximity, since those differ by far more
/// than 0.01.
const SEARCH_NEAR_SQL: &str = "SELECT gers_id, name, properties,
  ST_X(geom) AS x3857, ST_Y(geom) AS y3857,
  ST_X(ST_Transform(geom, 4326)) AS lon, ST_Y(ST_Transform(geom, 4326)) AS lat
FROM overture_places
WHERE name <> '' AND name ILIKE '%' || $1 || '%'
ORDER BY round(similarity(name, $1)::numeric, 2) DESC,
  geom <-> ST_Transform(ST_SetSRID(ST_MakePoint($3, $4), 4326), 3857) ASC
LIMIT $2";

/// `overture_divisions`/`overture_division_points` are small (a few thousand
/// and tens of thousands of rows nationwide) — no trigram index needed the way
/// `overture_places`' 691K rows requires one; a plain ILIKE scan over both is
/// fast enough as is.
///
/// Area first (`is_point` 0 before 1): a polygon's own centroid is a more
/// meaningful "center of this place" than a same-named point elsewhere would
/// be, and the caller dedupes by name keeping the first occurrence.
///
/// UNION's own ORDER BY can only reference output column names, not re-invoke
/// an expression like `similarity()` directly — confirmed live ("invalid
/// UNION/INTERSECT/EXCEPT ORDER BY clause") — so both branches compute their
/// own `sim` column and the ORDER BY references that instead.
///
/// x3857/y3857 alongside the WGS84 lon/lat: the client needs lon/lat to jump
/// the camera there, but resolving which region/county contains a result needs
/// the same point in the 3857 meters the divisions' geometry is stored in —
/// cheaper to select both once here than to reproject client-facing
/// coordinates back in application code.
const DIVISION_SEARCH_SQL: &str = "
(
  SELECT gers_id, name, subtype, names_common, 0 AS is_point, similarity(name, $1) AS sim,
    ST_X(ST_Centroid(geom)) AS x3857, ST_Y(ST_Centroid(geom)) AS y3857,
    ST_X(ST_Transform(ST_Centroid(geom), 4326)) AS lon,
    ST_Y(ST_Transform(ST_Centroid(geom), 4326)) AS lat
  FROM overture_divisions
  WHERE name <> '' AND name ILIKE '%' || $1 || '%'
)
UNION ALL
(
  SELECT gers_id, name, subtype, names_common, 1 AS is_point, similarity(name, $1) AS sim,
    ST_X(geom) AS x3857, ST_Y(geom) AS y3857,
    ST_X(ST_Transform(geom, 4326)) AS lon,
    ST_Y(ST_Transform(geom, 4326)) AS lat
  FROM overture_division_points
  WHERE name <> '' AND name ILIKE '%' || $1 || '%'
)
ORDER BY is_point ASC, sim DESC
LIMIT $2";

/// One matched place — WGS84 lon/lat, not a page index: unlike POI and reverse
/// geocode, both keyed to a finite L3 page a client already has open, search's
/// input space is unbounded free text and its output has to say exactly where
/// to fly the camera, not which page to look up.
#[derive(Serialize)]
pub struct SearchResult {
    pub gers_id: String,
    /// "poi" — an Overture Places business or venue. "division" — an
    /// administrative area matched by name.
    pub kind: &'static str,
    pub name: String,
    pub category: String,
    /// A language-tag-keyed name map, populated for a division (Overture's
    /// Divisions theme has genuinely rich per-language names) and always null
    /// for a POI (Places' own `names.common` is ~0% populated for Korean).
    pub common: Option<serde_json::Value>,
    /// The divisions containing this result's point, broad to fine. It answers
    /// "which of these is this one", for both kinds.
    pub context: Vec<GeocodePart>,
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

/// The same area/point name can legitimately show up twice (a `locality`
/// polygon and a `division` point both named "종로구", say) — keeping the
/// first occurrence is enough, since the query already orders area rows before
/// point rows for exactly this reason.
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

    // Enough division rows to survive the dedupe below, which can drop a name
    // that matched as both an area and a point.
    let division_fetch = DIVISION_LIMIT * 2;

    let (places_sql, places_params): (&str, Vec<&(dyn ToSql + Sync)>) = match near {
        Some(near) => (SEARCH_NEAR_SQL, vec![&query, &limit_i64, &near.lon, &near.lat]),
        None => (SEARCH_SQL, vec![&query, &limit_i64]),
    };
    let division_params: Vec<&(dyn ToSql + Sync)> = vec![&query, &division_fetch];

    let client = pool.get().await?;
    // One checkout, both statements. tokio-postgres pipelines on a single
    // connection, so the second goes out without waiting for the first to come
    // back; a second checkout would cost a connection per request to buy the
    // same overlap.
    let (places, divisions) = tokio::join!(client.query(places_sql, &places_params), client.query(DIVISION_SEARCH_SQL, &division_params),);

    let place_rows: Vec<PlaceRow> = places?.iter().map(PlaceRow::from_row).collect();
    let division_rows: Vec<DivisionRow> = dedupe_divisions_by_name(divisions?.iter().map(DivisionRow::from_row).collect())
        .into_iter()
        .take(DIVISION_LIMIT as usize)
        .collect();
    drop(client);

    // One extra query per division result, capped at DIVISION_LIMIT, so at
    // most a handful — each needs its own point-in-polygon lookup against a
    // different point.
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

    // Where the point actually is, rather than what a text field on the row
    // says about it.
    //
    // Overture's `addresses[0]` is contributor-entered and the geometry is
    // not. There is a row named 강남역 carrying the address "Seoul 서울특별시
    // 강남구" whose coordinates are in 용인, so the list said Seoul and picking
    // it moved the camera to 용인. That is not fixable in the address string.
    //
    // Nor is the inconsistency, which is the larger half: the same field
    // arrives as "서울 서초동 1319-13", as "Seoul Seocho-daero 398 Seocho-gu,
    // Seoul, Korea", as "Seoul", and as null, because each is however a
    // different contributor chose to write it. Resolving the point instead
    // gives every result the same shape in the reader's own language.
    let xs: Vec<f64> = place_rows.iter().map(|r| r.x3857).collect();
    let ys: Vec<f64> = place_rows.iter().map(|r| r.y3857).collect();
    let poi_context = geocode::resolve_point_context_batch(pool, &xs, &ys).await?;

    let poi_results = place_rows.into_iter().zip(poi_context).map(|(r, context)| SearchResult {
        gers_id: r.gers_id,
        name: r.name,
        kind: "poi",
        category: pxl_overture::properties::category_of(&r.properties),
        common: None,
        context,
        lon: r.lon,
        lat: r.lat,
    });

    // Divisions first: a query matching an admin-area name is rare enough to
    // almost always be the point, worth surfacing ahead of whatever POI
    // incidentally contains the same substring.
    Ok(division_results.into_iter().chain(poi_results).take(limit as usize).collect())
}
