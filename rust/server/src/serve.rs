// pxl-server/serve.rs

use std::process::ExitCode;

use axum::extract::DefaultBodyLimit;
use axum::http::StatusCode;
use axum::Router;
use deadpool_postgres::Pool;
use tower_http::compression::CompressionLayer;

use crate::{admin_auth, config, db};

pub struct Server<'a> {
    pub env: config::Env<'a>,
    pub probe: db::Probe<'a>,
    pub internal: &'static str,
}

pub fn shell(cfg: &config::Config, internal: &'static str, routes: Router) -> Router {
    // The gate covers the whole prefix. The explicit fallback is required: a
    // layer over a router with no routes never runs, and the request falls
    // through to the outer 404 ungated.
    let gated = Router::new()
        .fallback(|| async { StatusCode::NOT_FOUND })
        .layer(axum::middleware::from_fn_with_state(
            cfg.admin_token.clone(),
            admin_auth::admin_auth,
        ));
    routes
        .nest(internal, gated)
        .layer(CompressionLayer::new().gzip(true))
        .layer(DefaultBodyLimit::max(cfg.max_body_size))
}

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
