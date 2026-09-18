// pxl-geo-bake/geocode/tool/bake_geocode_tiles.rs

//! Offline pre-bake pass for reverse geocoding — the same shape as
//! bake_poi_tiles: for every land page, the neighbourhood label the provider
//! resolves for its centre, as one small gzipped JSON object. A page whose
//! centre falls outside every ingested division polygon — open water, or simply
//! outside Overture's Divisions coverage — writes an `.empty` marker instead.
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

use pxl_geo_bake::geocode::provider::overture_provider::GeocodeProvider;
use pxl_geo_bake::util::{fmt_bytes, git_sha, measure_output, now_iso8601};
use pxl_geo_bake::walk::{self, Args, Page};

struct Worker {
    client: Client,
    provider: GeocodeProvider,
}

/// What the two Divisions tables were when this ran. Read directly rather than
/// through the shared catalog helper, which returns every `overture_catalog`
/// row and not just this pipeline's two.
fn source(client: &mut Client) -> serde_json::Value {
    let rows = client
        .query(
            "SELECT file_name, data_date::text AS data_date, total_count FROM overture_catalog WHERE file_name IN ('overture_divisions', 'overture_division_points')",
            &[],
        )
        .unwrap_or_default();
    let mut out = serde_json::Map::new();
    for r in &rows {
        out.insert(
            r.get::<_, String>("file_name"),
            serde_json::json!({ "release": r.get::<_, Option<String>>("data_date"), "row_count": r.get::<_, i64>("total_count") }),
        );
    }
    serde_json::Value::Object(out)
}

fn run() -> Result<(), String> {
    let args = Args::parse(PathBuf::from(format!("./out/geocode/{}", PUBLISHED_VERSION.geocode)))?;
    println!(
        "[bake/geocode] out={} land_cache={} concurrency={} limit={} force={}",
        args.out.display(),
        args.land_cache.display(),
        args.concurrency,
        args.limit.map(|n| n.to_string()).unwrap_or_else(|| "none".into()),
        args.force
    );

    let url = std::env::var("DB_GEO_URL").map_err(|_| "env:DB_GEO_URL:missing")?;
    let walked = walk::walk(
        &args,
        "geocode",
        2000,
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
    )?;

    let t = &walked.tally;
    println!("[bake/geocode] gz={}", fmt_bytes(t.gz as f64));

    let mut client = Client::connect(&url, NoTls).map_err(|e| e.to_string())?;
    let (objects, gz_bytes) = measure_output(&args.out).map_err(|e| e.to_string())?;
    let manifest = serde_json::json!({
        "version": args.out.file_name().map(|n| n.to_string_lossy().to_string()),
        "updated_at": now_iso8601(),
        "git_sha": git_sha(),
        "land_pages": walked.land.len(),

        "objects": objects,
        "gz_bytes": gz_bytes,

        "run": { "baked": t.baked, "empty": t.empty, "skipped": t.skipped, "failed": t.failed, "gz_bytes": t.gz },

        "content_type": "application/json",
        "content_encoding": "gzip",

        "source": source(&mut client),
    });
    let path = args.out.join("../manifest.json");
    std::fs::write(&path, serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    println!("[bake/geocode] manifest -> {}", path.display());
    Ok(())
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
