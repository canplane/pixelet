// pxl-server/serve.test.rs

use axum::body::Body;
use axum::http::{Request, StatusCode};
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

const INTERNAL: &str = "/root/internal";

async fn get(uri: &str, token: Option<&str>) -> StatusCode {
    let mut req = Request::builder().uri(uri);
    if let Some(token) = token {
        req = req.header("x-admin-token", token);
    }
    shell(&cfg(), INTERNAL, Router::new()).oneshot(req.body(Body::empty()).unwrap()).await.unwrap().status()
}

/// The claim the gate is registered on: it covers the prefix rather than a
/// list of routes, so a path nobody has mounted is still behind the token.
/// Nothing is mounted under it today, which is exactly when this is easy to
/// get wrong and impossible to notice.
#[tokio::test]
async fn the_internal_prefix_is_gated_even_where_nothing_is_mounted() {
    assert_eq!(get("/root/internal/anything", None).await, StatusCode::UNAUTHORIZED);
    assert_eq!(get("/root/internal/anything", Some("wrong")).await, StatusCode::UNAUTHORIZED);

    // Past the gate there is genuinely nothing there, and saying so is the
    // right answer — a 404 here means the token was accepted.
    assert_eq!(get("/root/internal/anything", Some(TOKEN)).await, StatusCode::NOT_FOUND);
}

/// Nothing else is mounted, so everything outside the gated prefix is a plain
/// 404 — not a 401, which would say the gate reaches further than it does.
#[tokio::test]
async fn the_gate_does_not_reach_past_its_own_prefix() {
    assert_eq!(get("/root/anything", None).await, StatusCode::NOT_FOUND);
}
