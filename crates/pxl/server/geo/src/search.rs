// pxl-geo/search.rs

//! Forward search's own live endpoint.
//!
//! No cache layer, unlike the pages the bake produces: repeat identical
//! queries are cheap enough against a trigram-indexed column that a cache
//! would mostly add complexity to a feature whose whole point is that
//! *different* queries are the norm.

pub mod overture;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use deadpool_postgres::Pool;
use serde::Deserialize;

const MIN_QUERY_LEN: usize = 1;
const MAX_QUERY_LEN: usize = 100;
const DEFAULT_LIMIT: u32 = 20;
const MAX_LIMIT: u32 = 50;

#[derive(Deserialize)]
struct Params {
    q: Option<String>,
    near: Option<String>,
    limit: Option<String>,
}

pub struct Near {
    pub lon: f64,
    pub lat: f64,
}

fn parse_near(raw: Option<&str>) -> Option<Near> {
    let (lon, lat) = raw?.split_once(',')?;
    let lon: f64 = lon.parse().ok()?;
    let lat: f64 = lat.parse().ok()?;
    if !lon.is_finite() || !lat.is_finite() {
        return None;
    }
    Some(Near { lon, lat })
}

/// An unparseable `limit` is the default rather than a 400: it is a hint at
/// how much to return, and refusing the whole query over one would be
/// answering a smaller question with no answer at all.
fn parse_limit(raw: Option<&str>) -> u32 {
    raw.and_then(|v| v.parse::<u32>().ok()).filter(|n| *n > 0).unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT)
}

pub fn routes(pool: Pool) -> Router {
    // The branch the relays forward to this server rather than to the bucket.
    Router::new().route(pxl_spec::route::SEARCH_URL, get(search)).with_state(pool)
}

async fn search(State(pool): State<Pool>, Query(params): Query<Params>) -> Response {
    let q = params.q.unwrap_or_default().trim().to_string();
    // Characters rather than bytes: a Korean query is three bytes a syllable,
    // and a byte cap would reject a third of what this one allows.
    let len = q.chars().count();
    if !(MIN_QUERY_LEN..=MAX_QUERY_LEN).contains(&len) {
        return (StatusCode::BAD_REQUEST, axum::Json(serde_json::json!({ "error": "bad query" }))).into_response();
    }

    let near = parse_near(params.near.as_deref());
    let limit = parse_limit(params.limit.as_deref());

    match overture::search(&pool, &q, near.as_ref(), limit).await {
        Ok(results) => axum::Json(results).into_response(),
        Err(err) => {
            eprintln!("[search] db error: {err}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[cfg(test)]
#[path = "search.test.rs"]
mod test;
