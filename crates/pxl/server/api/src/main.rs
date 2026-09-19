// pxl-api/main.rs

//! The application's own server: sessions, chat, what a player places, and the
//! realtime half when it arrives.
//!
//! It answers nothing yet, and that is not the same as not existing. The
//! shape — a process that connects to db-main, probes it at boot and reserves
//! the internal prefix behind the admin token — is what a later route arrives
//! into. Taking it away
//! because no query read db-main yet would mean putting it back, along with the
//! root, the origin, the container and the secret, on the day the first route
//! is written.
//!
//! `/api/` is its root, one of the three spec/pxl/spec/route.json reserves. The world's
//! data is pxl-geo's `/geo/`, and neither server depends on the other: they read
//! separate databases and run for separate reasons — see
//! issue/20260918-the-server-in-rust-and-the-line-already-drawn.md.
//!
//! What does *not* belong here yet is a command schema or a payload shape. The
//! order those come in is recorded
//! (issue/20260910-point-density-and-the-order-to-multiplayer.md) and this is
//! not it: an address space is not a carrier, and fixing the second before the
//! storage model is settled is the failure that document names.

use std::process::ExitCode;

use axum::extract::DefaultBodyLimit;
use axum::http::StatusCode;
use axum::Router;
use pxl_server::{admin_auth, config, db};
use pxl_spec::route::API_INTERNAL;
use tower_http::compression::CompressionLayer;

fn router(cfg: &config::Config, _pool: deadpool_postgres::Pool) -> Router {
    // The internal prefix, gated, with nothing behind it — the same arrangement
    // pxl-geo has and for the same reason: the gate covers the prefix rather
    // than a list of routes, so whatever is added under it later is behind the
    // token by construction. The fallback is what makes that true; a layer over
    // a router with no routes of its own wraps nothing.
    let internal = Router::new()
        .fallback(|| async { StatusCode::NOT_FOUND })
        .layer(axum::middleware::from_fn_with_state(cfg.admin_token.clone(), admin_auth::admin_auth));

    Router::new()
        .nest(API_INTERNAL, internal)
        .layer(CompressionLayer::new().gzip(true))
        .layer(DefaultBodyLimit::max(cfg.max_body_sz))
}

async fn run() -> Result<(), db::DbError> {
    let cfg = config::Config::from_env(&config::Env {
        prefix: "API",
        default_port: 3002,
        db_url_key: "DB_MAIN_URL",
    })?;
    let pool = db::connect(&cfg.db_url, cfg.db_pool_size, &db::Probe { sql: "select 1", name: "main" }).await?;

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
