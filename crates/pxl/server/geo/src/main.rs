// pxl-geo/main.rs

//! Live forward search over Overture's Places and Divisions under `/geo/`;
//! a free-text query has no tile key, so it cannot be baked.

mod geocode;
mod search;

use std::process::ExitCode;

use pxl_server::config::Env;
use pxl_server::db::Probe;
use pxl_server::serve::{serve, Server};
use pxl_spec::route::GEO_INTERNAL;

#[tokio::main]
async fn main() -> ExitCode {
    let server = Server {
        env: Env {
            prefix: "GEO",
            default_port: 3000,
            db_url_key: "DB_GEO_URL",
        },
        probe: Probe {
            sql: "select postgis_full_version()",
            name: "geo",
        },
        internal: GEO_INTERNAL,
    };
    serve(server, search::endpoint::routes).await
}
