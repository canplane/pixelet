// pxl-geo-bake/poi/tool/bake_poi_tiles.rs

//! Offline pre-bake pass for POI: for every land page, the places the provider
//! finds there, encoded as the client's packet and gzipped. A page with none
//! writes an `.empty` marker instead of an object so a resumed run skips it
//! without asking the database again.
//!
//! The walk, the resume and the marker are walk.rs's, and its own comment
//! explains why terrain's land classification is what both page bakes filter
//! on.
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

use pxl_geo_bake::poi::provider::overture_provider::PoiProvider;
use pxl_geo_bake::util::{fmt_bytes, git_sha, measure_output, now_iso8601};
use pxl_geo_bake::walk::{self, Args, Page};

struct Worker {
    client: Client,
    provider: PoiProvider,
}

/// What `overture_places` was when this ran. Read directly rather than through
/// the shared catalog helper, which returns every `overture_catalog` row —
/// terrain's own tables included — and not just this pipeline's one.
fn source(client: &mut Client) -> serde_json::Value {
    let row = client
        .query_opt("SELECT data_date::text AS data_date, total_count FROM overture_catalog WHERE file_name = 'overture_places'", &[])
        .ok()
        .flatten();
    match row {
        Some(r) => serde_json::json!({
            "overture_places": { "release": r.get::<_, Option<String>>("data_date"), "row_count": r.get::<_, i64>("total_count") }
        }),
        None => serde_json::json!({ "overture_places": serde_json::Value::Null }),
    }
}

fn run() -> Result<(), String> {
    let args = Args::parse(PathBuf::from(format!("./out/poi/{}", PUBLISHED_VERSION.poi)))?;
    println!(
        "[bake/poi] out={} land_cache={} concurrency={} limit={} force={}",
        args.out.display(),
        args.land_cache.display(),
        args.concurrency,
        args.limit.map(|n| n.to_string()).unwrap_or_else(|| "none".into()),
        args.force
    );

    let url = std::env::var("DB_GEO_URL").map_err(|_| "env:DB_GEO_URL:missing")?;
    let walked = walk::walk(
        &args,
        "poi",
        500,
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
    )?;

    let t = &walked.tally;
    println!("[bake/poi] {} POI across {} non-empty pages, gz={}", t.items, t.baked, fmt_bytes(t.gz as f64));

    let mut client = Client::connect(&url, NoTls).map_err(|e| e.to_string())?;
    let (objects, gz_bytes) = measure_output(&args.out).map_err(|e| e.to_string())?;
    let manifest = serde_json::json!({
        "version": args.out.file_name().map(|n| n.to_string_lossy().to_string()),
        "updated_at": now_iso8601(),
        "git_sha": git_sha(),
        "land_pages": walked.land.len(),

        "objects": objects,
        "gz_bytes": gz_bytes,

        // This invocation only — a resumed run reports what it re-did, so a
        // zero here means "nothing to do" rather than "nothing wrong".
        "run": { "baked": t.baked, "empty": t.empty, "skipped": t.skipped, "failed": t.failed, "poi_total": t.items, "gz_bytes": t.gz },

        "content_type": "application/octet-stream",
        "content_encoding": "gzip",
        "format": "xtp poi page (encode_poi_page)",

        "source": source(&mut client),
    });
    let path = args.out.join("../manifest.json");
    std::fs::write(&path, serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    println!("[bake/poi] manifest -> {}", path.display());
    Ok(())
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
