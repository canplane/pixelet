// pxl-geo/main.rs

//! The live half of the geo data: forward search over Overture's Places and
//! Divisions. It is the one world feature that cannot be baked ahead of time
//! — terrain, POI and reverse geocode are each keyed to a finite L3 page and
//! are served as static objects out of the bucket, while a free-text query
//! has no such key and has to reach Postgres at request time.
//!
//! What this crate is not is the whole server. `/api/` is one root, and
//! everything the server answers sits under it
//! (issue/20260910-one-server-root.md); the crate that will hold sessions,
//! chat and the realtime half is named for being the application's server
//! rather than for owning that prefix, and it answers under the same root.
//! The two are separate processes because they read separate databases —
//! see issue/20260918-the-server-in-rust-and-the-line-already-drawn.md.

mod admin_auth;
mod config;
mod db;
mod geocode;
mod search;

use std::process::ExitCode;

use axum::extract::DefaultBodyLimit;
use axum::http::StatusCode;
use axum::Router;
use tower_http::compression::CompressionLayer;

fn router(cfg: &config::Config, pool: deadpool_postgres::Pool) -> Router {
    // Nothing is mounted under the internal prefix yet — the cache
    // invalidation routes it was built for belong to the bake's own dev
    // server, not here. The gate is registered anyway so that it covers the
    // prefix rather than a list of routes: whatever is added under it later is
    // behind the token by construction.
    //
    // The fallback is what makes that true, and it is not decoration. A layer
    // over a router with no routes of its own wraps nothing — the request falls
    // through to the outer router's 404 without the middleware ever running,
    // which is what this did until main.test.rs said so. An explicit fallback
    // gives the layer something to wrap, so the prefix is answered here, and
    // answered behind the token.
    let internal = Router::new()
        .fallback(|| async { StatusCode::NOT_FOUND })
        .layer(axum::middleware::from_fn_with_state(cfg.admin_token.clone(), admin_auth::admin_auth));

    Router::new()
        .merge(search::routes(pool))
        .nest("/api/internal", internal)
        // Search answers JSON, which compresses well and which the browser
        // decodes transparently.
        .layer(CompressionLayer::new().gzip(true))
        .layer(DefaultBodyLimit::max(cfg.max_body_sz))
}

async fn run() -> Result<(), db::DbError> {
    let cfg = config::Config::from_env()?;
    let pool = db::connect(&cfg.db_geo_url, cfg.db_pool_size).await?;

    let listener = tokio::net::TcpListener::bind((cfg.host.as_str(), cfg.port)).await?;
    println!("[app] listening on {}:{}", cfg.host, cfg.port);

    axum::serve(listener, router(&cfg, pool)).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("[app] {err}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
#[path = "main.test.rs"]
mod test;
