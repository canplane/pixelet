// pxl-server/admin_auth.rs

use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

/// Shared-secret gate for internal routes; not real auth.
pub async fn admin_auth(State(token): State<String>, req: Request, next: Next) -> Response {
    if req.headers().get("x-admin-token").and_then(|v| v.to_str().ok()) != Some(token.as_str()) {
        return (StatusCode::UNAUTHORIZED, "unauthorized").into_response();
    }
    next.run(req).await
}
