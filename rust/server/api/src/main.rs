// pxl-api/main.rs

//! The application's server, for sessions, chat and placed objects. It holds the db-main
//! connection and the gated internal prefix, and serves no routes.

use std::process::ExitCode;

use axum::Router;
use pxl_server::config::Env;
use pxl_server::db::Probe;
use pxl_server::serve::{Server, serve};
use pxl_spec::route::API_INTERNAL;

#[tokio::main]
async fn main() -> ExitCode {
    let server = Server {
        env: Env {
            prefix: "API",
            db_url_key: "DB_MAIN_URL",
        },
        probe: Probe {
            sql: "select 1",
            name: "main",
        },
        internal: API_INTERNAL,
    };
    serve(server, |_pool| Router::new()).await
}
