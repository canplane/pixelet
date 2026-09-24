// pxl-api/main.test.rs

use axum::body::Body;
use axum::http::{Request, StatusCode};
use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use tokio_postgres::NoTls;
use tower::ServiceExt;

use super::*;

const TOKEN: &str = "a-token";

fn cfg() -> config::Config {
    config::Config {
        host: "127.0.0.1".to_string(),
        port: 0,
        max_body_sz: 1 << 20,
        admin_token: TOKEN.to_string(),
        db_url: String::new(),
        db_pool_size: 1,
    }
}

/// deadpool connects lazily, and no path exercised here reaches the database —
/// which is the point, since the gate has to pass with no db-main up.
fn unreachable_pool() -> Pool {
    let pg: tokio_postgres::Config = "postgresql://nobody@127.0.0.1:1/nothing".parse().unwrap();
    let mgr = Manager::from_config(
        pg,
        NoTls,
        ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        },
    );
    Pool::builder(mgr).max_size(1).build().unwrap()
}

async fn get(uri: &str, token: Option<&str>) -> StatusCode {
    let mut req = Request::builder().uri(uri);
    if let Some(token) = token {
        req = req.header("x-admin-token", token);
    }
    router(&cfg(), unreachable_pool()).oneshot(req.body(Body::empty()).unwrap()).await.unwrap().status()
}

/// The claim the gate is registered on: it covers the prefix rather than a
/// list of routes, so a path nobody has mounted is still behind the token.
/// Nothing is mounted under it today, which is exactly when this is easy to
/// get wrong and impossible to notice.
#[tokio::test]
async fn the_internal_prefix_is_gated_even_where_nothing_is_mounted() {
    assert_eq!(get("/api/internal/anything", None).await, StatusCode::UNAUTHORIZED);
    assert_eq!(get("/api/internal/anything", Some("wrong")).await, StatusCode::UNAUTHORIZED);

    // Past the gate there is genuinely nothing there, and saying so is the
    // right answer — a 404 here means the token was accepted.
    assert_eq!(get("/api/internal/anything", Some(TOKEN)).await, StatusCode::NOT_FOUND);
}

/// Nothing else is mounted, so everything outside the gated prefix is a plain
/// 404 — not a 401, which would say the gate reaches further than it does.
#[tokio::test]
async fn the_gate_does_not_reach_past_its_own_prefix() {
    assert_eq!(get("/api/anything", None).await, StatusCode::NOT_FOUND);
}
