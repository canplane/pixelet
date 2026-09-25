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

use axum::Router;
use pxl_server::config::Env;
use pxl_server::db::Probe;
use pxl_server::serve::{serve, Server};
use pxl_spec::route::API_INTERNAL;

#[tokio::main]
async fn main() -> ExitCode {
    let server = Server {
        env: Env {
            prefix: "API",
            default_port: 3002,
            db_url_key: "DB_MAIN_URL",
        },
        probe: Probe { sql: "select 1", name: "main" },
        internal: API_INTERNAL,
    };
    // No route yet: the shell's gated prefix is all it answers.
    serve(server, |_pool| Router::new()).await
}
