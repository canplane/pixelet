// pxl-geo/admin_auth.rs

use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

/// Shared-secret gate for low-traffic internal/ops routes — not a real auth
/// system, just enough to keep them from being callable by anyone who finds
/// the URL.
pub async fn admin_auth(State(token): State<String>, req: Request, next: Next) -> Response {
    if req.headers().get("x-admin-token").and_then(|v| v.to_str().ok()) != Some(token.as_str()) {
        return (StatusCode::UNAUTHORIZED, "unauthorized").into_response();
    }
    next.run(req).await
}
