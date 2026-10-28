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
    shell(&cfg(), INTERNAL, Router::new())
        .oneshot(req.body(Body::empty()).unwrap())
        .await
        .unwrap()
        .status()
}

#[tokio::test]
async fn internal_prefix_gated() {
    assert_eq!(
        get("/root/internal/anything", None).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        get("/root/internal/anything", Some("wrong")).await,
        StatusCode::UNAUTHORIZED
    );

    // 404 past the gate means the token was accepted.
    assert_eq!(
        get("/root/internal/anything", Some(TOKEN)).await,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn gate_scoped_to_prefix() {
    assert_eq!(get("/root/anything", None).await, StatusCode::NOT_FOUND);
}
