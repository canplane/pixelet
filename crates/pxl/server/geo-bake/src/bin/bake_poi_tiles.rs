// pxl-geo-bake/bin/bake_poi_tiles.rs

//! Offline pre-bake pass for POI: for every land page, the places the provider
//! finds there, encoded as the client's packet and gzipped. A page with none
//! writes an `.empty` marker instead of an object so a resumed run skips it
//! without asking the database again.
//!
//! The walk, the resume, the marker and the manifest are walk.rs's, which
//! reverse geocoding goes through too — what is here is the provider and the
//! encoding, which is all the two differ in.
//!
//! Usage (from the repo root):
//!   cargo run --profile server -p pxl-geo-bake --bin bake_poi_tiles -- [flags]
//!
//! Flags:
//!   --out <dir>          output directory (default ./out/poi/<published version>)
//!   --land-cache <path>  terrain's own land_pages.json (default ./out/terrain/land_pages.json)
//!   --concurrency <n>    parallel fetches, one connection each (default 8)
//!   --limit <n>          stop after n land pages (smoke test)
//!   --force              re-fetch pages that already have an object or a marker

use std::path::PathBuf;
use std::process::ExitCode;

use postgres::{Client, NoTls};
use pxl_spec::bake::format::encode_poi_page;
use pxl_spec::bake::published::PUBLISHED_VERSION;

use pxl_geo_bake::page::bake::{self, Bake};
use pxl_geo_bake::page::walk::Page;
use pxl_geo_bake::poi::provider::overture_provider::PoiProvider;
use pxl_geo_spec::table;

struct Worker {
    client: Client,
    provider: PoiProvider,
}

fn run() -> Result<(), String> {
    let url = std::env::var("DB_GEO_URL").map_err(|_| "env:DB_GEO_URL:missing")?;
    bake::bake(
        &Bake {
            kind: "poi",
            out: PathBuf::from(format!("./out/poi/{}", PUBLISHED_VERSION.poi)),
            every: 500,
            content_type: "application/octet-stream",
            format: Some("xtp poi page (encode_poi_page)"),
            sources: &[table::PLACES],
        },
        || {
            let mut client = Client::connect(&url, NoTls).map_err(|e| e.to_string())?;
            let provider = PoiProvider::prepare(&mut client).map_err(|e| e.to_string())?;
            Ok(Worker { client, provider })
        },
        |w, page_idx| {
            let pois = w.provider.fetch_pois(&mut w.client, page_idx).map_err(|e| e.to_string())?;
            if pois.is_empty() {
                return Ok(Page::Empty);
            }
            // The page as the client's own packet — lanes it views in place,
            // strings it reads one at a time — gzipped like terrain.
            let raw = encode_poi_page(&pois).map_err(|e| format!("{e:?}"))?;
            Ok(Page::Object {
                raw: raw.as_slice().to_vec(),
                items: pois.len() as u64,
            })
        },
    )
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("[bake/poi] {err}");
            ExitCode::FAILURE
        }
    }
}
