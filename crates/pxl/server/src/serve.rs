// pxl-server/serve.rs

//! A server's whole life, the same for both: its config read from the
//! environment, its database probed before the port opens, its own routes
//! inside the shell every server here wears, and a failure anywhere as the
//! process's exit. What the routes are is the server's.

use std::process::ExitCode;

use axum::extract::DefaultBodyLimit;
use axum::http::StatusCode;
use axum::Router;
use deadpool_postgres::Pool;
use tower_http::compression::CompressionLayer;

use crate::{admin_auth, config, db};

/// What tells one server from the other before a route is written: the
/// environment it reads, the database it proves reachable and the internal
/// prefix its root reserves (spec/pxl/spec/route.json).
pub struct Server<'a> {
    pub env: config::Env<'a>,
    pub probe: db::Probe<'a>,
    pub internal: &'static str,
}

/// A server's own routes inside the shell: the internal prefix behind the
/// admin token, and compression and the body limit over all of it.
pub fn shell(cfg: &config::Config, internal: &'static str, routes: Router) -> Router {
    // The gate covers the prefix rather than a list of routes, so whatever is
    // added under it later is behind the token by construction — and nothing
    // is mounted under it today.
    //
    // The fallback is what makes that true, and it is not decoration. A layer
    // over a router with no routes of its own wraps nothing — the request falls
    // through to the outer router's 404 without the middleware ever running,
    // which is what the gate did until a test said so. An explicit fallback
    // gives the gated router a route of its own, so every path under the
    // prefix passes the gate before it is answered.
    let gated = Router::new()
        .fallback(|| async { StatusCode::NOT_FOUND })
        .layer(axum::middleware::from_fn_with_state(cfg.admin_token.clone(), admin_auth::admin_auth));
    routes
        .nest(internal, gated)
        // What answers is JSON, which compresses well and which the browser
        // decodes transparently.
        .layer(CompressionLayer::new().gzip(true))
        .layer(DefaultBodyLimit::max(cfg.max_body_sz))
}

/// Runs the server until it stops; the exit it stopped with.
pub async fn serve(server: Server<'_>, routes: impl FnOnce(Pool) -> Router) -> ExitCode {
    match run(server, routes).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("[app] {err}");
            ExitCode::FAILURE
        }
    }
}

async fn run(server: Server<'_>, routes: impl FnOnce(Pool) -> Router) -> Result<(), db::DbError> {
    let cfg = config::Config::from_env(&server.env)?;
    let pool = db::connect(&cfg.db_url, cfg.db_pool_size, &server.probe).await?;
    let listener = tokio::net::TcpListener::bind((cfg.host.as_str(), cfg.port)).await?;
    println!("[app] listening on {}:{}", cfg.host, cfg.port);
    axum::serve(listener, shell(&cfg, server.internal, routes(pool))).await?;
    Ok(())
}

#[cfg(test)]
#[path = "serve.test.rs"]
mod test;
