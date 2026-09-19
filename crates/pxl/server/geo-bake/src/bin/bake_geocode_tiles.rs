// pxl-geo-bake/bin/bake_geocode_tiles.rs

//! Offline pre-bake pass for reverse geocoding: for every land page, the
//! neighbourhood label the provider resolves for its centre, as one small
//! gzipped JSON object. A page whose centre falls outside every ingested
//! division polygon — open water, or simply outside Overture's Divisions
//! coverage — writes an `.empty` marker instead.
//!
//! The same shape as bake_poi_tiles, and through the same walk: the provider
//! and the encoding are what differ.
//!
//! Usage (from the repo root):
//!   cargo run --profile server -p pxl-geo-bake --bin bake_geocode_tiles -- [flags]
//!
//! Flags:
//!   --out <dir>          output directory (default ./out/geocode/<published version>)
//!   --land-cache <path>  terrain's own land_pages.json (default ./out/terrain/land_pages.json)
//!   --concurrency <n>    parallel fetches, one connection each (default 8)
//!   --limit <n>          stop after n land pages (smoke test)
//!   --force              re-fetch pages that already have an object or a marker

use std::path::PathBuf;
use std::process::ExitCode;

use postgres::{Client, NoTls};
use pxl_spec::bake::published::PUBLISHED_VERSION;

use pxl_geo_bake::geocode::provider::overture::GeocodeProvider;
use pxl_geo_bake::page::bake::{self, Bake};
use pxl_geo_bake::page::walk::Page;
use pxl_geo_spec::table;

struct Worker {
    client: Client,
    provider: GeocodeProvider,
}

fn run() -> Result<(), String> {
    let url = std::env::var("DB_GEO_URL").map_err(|_| "env:DB_GEO_URL:missing")?;
    bake::bake(
        &Bake {
            kind: "geocode",
            out: PathBuf::from(format!("./out/geocode/{}", PUBLISHED_VERSION.geocode)),
            every: 2000,
            content_type: "application/json",
            format: None,
            sources: &[table::DIVISIONS, table::DIVISION_POINTS],
        },
        || {
            let mut client = Client::connect(&url, NoTls).map_err(|e| e.to_string())?;
            let provider = GeocodeProvider::prepare(&mut client).map_err(|e| e.to_string())?;
            Ok(Worker { client, provider })
        },
        |w, page_idx| match w.provider.reverse(&mut w.client, page_idx).map_err(|e| e.to_string())? {
            None => Ok(Page::Empty),
            Some(result) => Ok(Page::Object {
                raw: serde_json::to_vec(&result).map_err(|e| e.to_string())?,
                items: result.parts.len() as u64,
            }),
        },
    )
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("[bake/geocode] {err}");
            ExitCode::FAILURE
        }
    }
}
