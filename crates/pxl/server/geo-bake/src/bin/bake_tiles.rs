// pxl-geo-bake/bin/bake_tiles.rs

//! Offline pre-bake pass: walks every L3 page inside the geo DB's coverage
//! bbox, skips the ones that are pure open ocean, and bakes the rest into
//! packed tile objects (util.rs's CONTENT_ENCODING) ready to upload to a static object store and be served
//! straight to the client with no API server in the loop.
//!
//! Land/ocean classification is a coarse pre-filter, cheaper and separate from
//! the bake path's own fine-grained coastline carving. It checks
//! `overture_base_land` directly for existence, not the full carve; a page with
//! zero touching land rows is open ocean and gets served by the client's single
//! shared fallback tile instead.
//!
//! Tile bytes come from terrain/source.rs, the same assembly the dev endpoint
//! uses, so a bucket tile and a locally served one are identical byte for byte.
//!
//! Those are the leaves, z16, under `<out>/0/`. Every level above is folded
//! from the one below it on disk (pyramid.rs) into `<out>/<level>/`,
//! with no database in the loop, up to TOP_LEVEL. The levels are
//! always refolded whole: they are a function of the leaves, so a resumed or
//! partial leaf bake is never left under a stale pyramid.
//!
//! Usage (from the repo root):
//!   cargo run --profile server -p pxl-geo-bake --bin bake_tiles -- [flags]
//!
//! Flags:
//!   --out <dir>          output directory (default ./out/terrain/<published version>)
//!   --concurrency <n>    parallel bakes, one connection each (default 8)
//!   --limit <n>          stop after n land tiles (smoke test)
//!   --force              re-bake tiles that already exist on disk
//!   --reclassify         ignore a cached land_pages.json and re-run the scan

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use postgres::{Client, NoTls};
use pxl_spec::bake::format::Terrain;
use pxl_spec::bake::published::PUBLISHED_VERSION;
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pyramid::LevelPage;

use pxl_geo_bake::cli::Flags;
use pxl_geo_bake::page::walk::{land_pages, write_land_pages};
use pxl_geo_bake::pyramid::{build, write_manifest, Layer, PageSet};
use pxl_geo_bake::terrain::leaf::{bake_leaf, bake_ocean, has_land, ocean_samples, Coverage, NE, OCEAN_KEY, SW};
use pxl_geo_bake::terrain::repository::TerrainRepository;
use pxl_geo_bake::terrain::source::SourceStats;
use pxl_geo_bake::util::{fmt_bytes, fmt_dur, pack, refuse_mixed, Level};

const CLASSIFY_BATCH: usize = 2000;

// ---- args ----

struct Args {
    out: PathBuf,
    concurrency: usize,
    limit: Option<usize>,
    force: bool,
    reclassify: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        out: PathBuf::from(format!("./out/terrain/{}", PUBLISHED_VERSION.terrain)),
        concurrency: 8,
        limit: None,
        force: false,
        reclassify: false,
    };

    let mut flags = Flags::from_env();
    while let Some(flag) = flags.arg() {
        match flag.as_str() {
            "--out" => args.out = PathBuf::from(flags.value()?),
            "--concurrency" => args.concurrency = flags.parse()?,
            "--limit" => args.limit = Some(flags.parse()?),
            "--force" => args.force = true,
            "--reclassify" => args.reclassify = true,
            other => return Err(format!("unknown flag: {other}")),
        }
    }
    Ok(args)
}

// ---- helpers ----

/// Snapshot of what the geo DB was holding when a version was baked. A URL
/// prefix like `v1` is a cache key — deliberately short and stable, since
/// putting a date in it would drag the app build along on every re-bake. That
/// leaves it opaque about *what* v1 actually is, which is what this records
/// instead.
fn read_source_catalogs(client: &mut Client) -> serde_json::Value {
    let mut out = serde_json::Map::new();
    for table in ["overture_catalog", "elevation_catalog"] {
        let sql = format!("SELECT region_code, data_date::text AS data_date, file_name, status FROM {table} ORDER BY file_name");
        let rows = match client.query(sql.as_str(), &[]) {
            Ok(rows) => rows,
            Err(err) => {
                // A missing catalog table is not fatal to the bake — record the
                // gap rather than pretending the provenance is complete.
                eprintln!("[bake/provenance] could not read {table}: {err}");
                out.insert(table.to_string(), serde_json::Value::Array(vec![]));
                continue;
            }
        };
        let list: Vec<serde_json::Value> = rows
            .iter()
            .map(|r| {
                serde_json::json!({
                    "region_code": r.get::<_, String>("region_code"),
                    "data_date": r.get::<_, Option<String>>("data_date"),
                    "file_name": r.get::<_, String>("file_name"),
                    "status": r.get::<_, String>("status"),
                })
            })
            .collect();
        out.insert(table.to_string(), serde_json::Value::Array(list));
    }
    serde_json::Value::Object(out)
}

// ---- classification ----

fn classify_land_pages(client: &mut Client, candidates: &[IVec]) -> Result<Vec<IVec>, postgres::Error> {
    let mut land: Vec<IVec> = Vec::new();
    for (at, batch) in candidates.chunks(CLASSIFY_BATCH).enumerate() {
        for (&page, touches) in batch.iter().zip(has_land(client, batch)?) {
            if touches {
                land.push(page);
            }
        }
        let done = (at * CLASSIFY_BATCH + batch.len()).min(candidates.len());
        println!("[bake/classify] {done}/{} land={}", candidates.len(), land.len());
    }
    Ok(land)
}

// ---- run ----

#[derive(Default)]
struct Totals {
    baked: u64,
    skipped: u64,
    failed: u64,
    raw: u64,
    bytes: u64,
    invalid_ele_shape: u64,
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    let tiles = PageSet::new(&args.out);
    println!(
        "[bake] out={} concurrency={} limit={} force={}",
        args.out.display(),
        args.concurrency,
        args.limit.map(|n| n.to_string()).unwrap_or_else(|| "none".into()),
        args.force
    );
    fs::create_dir_all(tiles.level_dir(0)).map_err(|e| e.to_string())?;
    // The levels above are refolded whole; the leaves and the ocean page are
    // what a resumed run keeps.
    if !args.force {
        refuse_mixed(&tiles.root)?;
        refuse_mixed(&tiles.level_dir(0))?;
    }

    let url = std::env::var("DB_GEO_URL").map_err(|_| "env:DB_GEO_URL:missing")?;
    let mut client = Client::connect(&url, NoTls).map_err(|e| e.to_string())?;

    let coverage = Coverage::of_source();
    let candidates = coverage.pages();
    println!(
        "[bake] page grid x[{}..{}] y[{}..{}] = {} candidate pages",
        coverage.x.0,
        coverage.x.1,
        coverage.y.0,
        coverage.y.1,
        candidates.len()
    );

    // ---- classify (cached: the scan is deterministic and slow-ish) ----
    let land_cache = args.out.join("../land_pages.json");
    // A list classified at another page size is not reusable, and this is the
    // one bake that can classify again, so a refused cache is simply redone.
    let cached = if args.reclassify { Err("--reclassify".to_string()) } else { land_pages(&land_cache) };
    let land: Vec<IVec> = match cached {
        Ok(land) => {
            println!("[bake/classify] reusing cached list: {} land pages ({})", land.len(), land_cache.display());
            land
        }
        Err(why) => {
            println!("[bake/classify] classifying: {why}");
            let land = classify_land_pages(&mut client, &candidates).map_err(|e| e.to_string())?;
            write_land_pages(&land_cache, &land)?;
            println!("[bake/classify] done: land={} ocean={} -> {}", land.len(), candidates.len() - land.len(), land_cache.display());
            land
        }
    };
    if land.is_empty() {
        return Err("no land pages: nothing to bake".to_string());
    }

    // ---- ocean fallback tile ----
    let ocean_path = tiles.root.join(OCEAN_KEY);
    if args.force || !ocean_path.exists() {
        let ocean_raw = bake_ocean(&ocean_samples(&coverage))?;
        let ocean_packed = pack(ocean_raw.as_slice(), Level::Stored).map_err(|e| e.to_string())?;
        fs::write(&ocean_path, &ocean_packed).map_err(|e| e.to_string())?;
        println!(
            "[bake/ocean] wrote {OCEAN_KEY}: raw={} packed={} (page-independent: verified)",
            fmt_bytes(ocean_raw.len() as f64),
            fmt_bytes(ocean_packed.len() as f64)
        );
    }

    // ---- bake land tiles ----
    let targets: &[IVec] = match args.limit {
        Some(n) => &land[..n.min(land.len())],
        None => &land,
    };

    let next = AtomicUsize::new(0);
    let totals = Mutex::new(Totals::default());
    let started = Instant::now();

    // A worker owns its connection and its prepared statements for the whole
    // run, which is what the five-queries-on-one-client shape wants; page-level
    // concurrency and the connection count are then the same number.
    std::thread::scope(|scope| {
        for _ in 0..args.concurrency.max(1) {
            scope.spawn(|| {
                let mut client = match Client::connect(&url, NoTls) {
                    Ok(c) => c,
                    Err(err) => return eprintln!("[bake/worker] connect failed: {err}"),
                };
                let repo = match TerrainRepository::prepare(&mut client) {
                    Ok(r) => r,
                    Err(err) => return eprintln!("[bake/worker] prepare failed: {err}"),
                };
                let mut stats = SourceStats::default();

                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(&page_idx) = targets.get(i) else { break };
                    if !args.force && tiles.path(LevelPage::new(0, page_idx)).exists() {
                        totals.lock().expect("totals").skipped += 1;
                        continue;
                    }

                    let before = stats.invalid_ele_shape;
                    let result =
                        bake_leaf(&repo, &mut client, page_idx, &mut stats, Level::Stored).and_then(|(raw, packed)| tiles.write(LevelPage::new(0, page_idx), &packed).map(|()| (raw, packed.len())));

                    let done = {
                        let mut t = totals.lock().expect("totals");
                        match result {
                            Ok((raw, bytes)) => {
                                t.baked += 1;
                                t.raw += raw as u64;
                                t.bytes += bytes as u64;
                            }
                            Err(err) => {
                                t.failed += 1;
                                eprintln!("[bake/tile/{}] failed: {err}", ivec::repr(page_idx));
                            }
                        }
                        t.invalid_ele_shape += stats.invalid_ele_shape - before;
                        t.baked + t.skipped + t.failed
                    };

                    if done.is_multiple_of(200) {
                        let t = totals.lock().expect("totals");
                        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
                        let rate = if t.baked > 0 { elapsed / t.baked as f64 } else { 0.0 };
                        println!(
                            "[bake] {done}/{} baked={} skipped={} failed={} packed={} {rate:.0}ms/tile eta={}",
                            targets.len(),
                            t.baked,
                            t.skipped,
                            t.failed,
                            fmt_bytes(t.bytes as f64),
                            fmt_dur(rate * (targets.len() as u64 - done) as f64)
                        );
                    }
                }
            });
        }
    });

    let t = totals.into_inner().expect("totals");
    let elapsed = started.elapsed().as_secs_f64() * 1000.0;
    let avg = if t.baked > 0 { t.bytes as f64 / t.baked as f64 } else { 0.0 };

    println!();
    println!("[bake] done in {}: baked={} skipped={} failed={}", fmt_dur(elapsed), t.baked, t.skipped, t.failed);
    println!(
        "[bake] raw={} packed={} (avg {}/tile packed, {:.1}% of raw)",
        fmt_bytes(t.raw as f64),
        fmt_bytes(t.bytes as f64),
        fmt_bytes(avg),
        t.bytes as f64 / t.raw.max(1) as f64 * 100.0
    );
    println!("[bake] projected full set ({} land tiles): {}", land.len(), fmt_bytes(avg * land.len() as f64));

    // A degraded tile still writes successfully, so it never shows up in
    // `failed` — surface it here rather than leaving it buried in per-page
    // warnings that nobody greps for after a forty-minute run.
    if t.invalid_ele_shape > 0 {
        let pct = t.invalid_ele_shape as f64 / t.baked.max(1) as f64 * 100.0;
        eprintln!(
            "[bake] WARNING: {} of {} tiles ({pct:.2}%) had an invalid elevation shape and were baked FLAT (zero-filled elevation). \
             Expected at the outer edge of the DEM coverage, where the padded sampling window falls outside the source raster.",
            t.invalid_ele_shape, t.baked
        );
    }

    // ---- the levels above ----
    let ocean = Terrain::decode(&pxl_geo_bake::util::unpack(&fs::read(&ocean_path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?)?;
    let top = build(&tiles, &ocean, args.concurrency)?;

    write_manifest(
        &tiles,
        top,
        serde_json::json!({
            "bbox_4326": { "sw": SW, "ne": NE },
            "candidate_pages": candidates.len(),
            "land_pages": land.len(),
            "ocean_pages": candidates.len() - land.len(),
            "ocean_key": OCEAN_KEY,
            // This invocation only — a resumed run reports only what it
            // re-baked, so a zero here means "nothing to do", not "nothing
            // wrong".
            "run": {
                "baked": t.baked,
                "skipped": t.skipped,
                "failed": t.failed,
                "bytes": t.bytes,
                "flat_elevation_tiles": t.invalid_ele_shape,
            },
            "source": read_source_catalogs(&mut client),
        }),
    )?;

    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("[bake] {err}");
            ExitCode::FAILURE
        }
    }
}
