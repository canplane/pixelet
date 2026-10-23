// pxl-geo/search/endpoint.test.rs

//! A deadpool pool connects lazily, so these run with no geo DB up.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use tokio_postgres::NoTls;
use tower::ServiceExt;

use super::*;

fn unreachable_pool() -> Pool {
    let cfg: tokio_postgres::Config = "postgresql://nobody@127.0.0.1:1/nothing".parse().unwrap();
    let mgr = Manager::from_config(
        cfg,
        NoTls,
        ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        },
    );
    Pool::builder(mgr).max_size(1).build().unwrap()
}

async fn get(uri: &str) -> StatusCode {
    routes(unreachable_pool()).oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap()).await.unwrap().status()
}

#[tokio::test]
async fn a_query_outside_the_length_bounds_is_refused_before_the_database() {
    assert_eq!(get("/geo/search").await, StatusCode::BAD_REQUEST);
    assert_eq!(get("/geo/search?q=").await, StatusCode::BAD_REQUEST);
    assert_eq!(get("/geo/search?q=%20%20").await, StatusCode::BAD_REQUEST);

    let over = "a".repeat(MAX_QUERY_LEN + 1);
    assert_eq!(get(&format!("/geo/search?q={over}")).await, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn the_length_bound_counts_characters_rather_than_bytes() {
    // 100 three-byte characters are 300 bytes; a byte cap would refuse them.
    let wide = "€".repeat(MAX_QUERY_LEN);
    assert_ne!(get(&format!("/geo/search?q={wide}")).await, StatusCode::BAD_REQUEST);

    let over = "€".repeat(MAX_QUERY_LEN + 1);
    assert_eq!(get(&format!("/geo/search?q={over}")).await, StatusCode::BAD_REQUEST);
}

#[test]
fn near_takes_a_pair_of_finite_numbers_and_nothing_else() {
    let near = parse_near(Some("127.03,37.5")).unwrap();
    assert_eq!((near.lon, near.lat), (127.03, 37.5));

    assert!(parse_near(None).is_none());
    assert!(parse_near(Some("127.03")).is_none());
    assert!(parse_near(Some("127.03,37.5,12")).is_none());
    assert!(parse_near(Some("east,north")).is_none());
    assert!(parse_near(Some("inf,37.5")).is_none());
    assert!(parse_near(Some("NaN,37.5")).is_none());
}

#[test]
fn limit_is_clamped_and_a_bad_one_is_the_default_rather_than_a_refusal() {
    assert_eq!(parse_limit(Some("5")), 5);
    assert_eq!(parse_limit(None), DEFAULT_LIMIT);
    assert_eq!(parse_limit(Some("0")), DEFAULT_LIMIT);
    assert_eq!(parse_limit(Some("many")), DEFAULT_LIMIT);
    assert_eq!(parse_limit(Some("-3")), DEFAULT_LIMIT);
    assert_eq!(parse_limit(Some("999")), MAX_LIMIT);
}
