// pxl-geo/main.rs

//! The live half of the geo data: forward search over Overture's Places and
//! Divisions. It is the one world feature that cannot be baked ahead of time
//! — terrain, POI and reverse geocode are each keyed to a finite L3 page and
//! are served as static objects out of the bucket, while a free-text query
//! has no such key and has to reach Postgres at request time.
//!
//! It answers under `/geo/`, one of the three roots spec/pxl/spec/route.json reserves:
//! the world's data. The application's is `/api/`, which pxl-api answers, and
//! the assets have their own. Each root is one prefix a relay reads and never a
//! route it enumerates (issue/20260910-one-server-root.md), and the two servers
//! are separate processes because they read separate databases — see
//! issue/20260918-the-server-in-rust-and-the-line-already-drawn.md.

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
