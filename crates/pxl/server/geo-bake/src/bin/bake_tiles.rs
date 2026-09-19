// pxl-geo-bake/bin/bake_tiles.rs

//! Offline pre-bake pass: walks every L3 page inside the geo DB's coverage
//! bbox, skips the ones that are pure open ocean, and bakes the rest into
//! gzipped tile objects ready to upload to a static object store and be served
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
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use postgres::types::ToSql;
use postgres::{Client, NoTls};
use pxl_spec::bake::constant::ELE_PADDED_SPAN;
use pxl_spec::bake::published::PUBLISHED_VERSION;
use pxl_spec::space::adapter::{epsg3857, epsg4326};
use pxl_spec::space::indexer::L3_IDX;
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pvf3::pvf3::Pvf3;
use xpute_core::wire::xtp::{encoder, BranchView, NodeView, TreeEncoderOptions, TreeView};

use pxl_geo_bake::terrain::bake::bake_terrain;
use pxl_geo_bake::terrain::repository::TerrainRepository;
use pxl_geo_bake::terrain::source::{fetch_tile, SourceStats};
use pxl_geo_bake::util::{fmt_bytes, fmt_dur, git_sha, gzip, measure_output, now_iso8601, write_atomic};

/// Current `elevation_raster` coverage bbox — confirmed with
/// `SELECT ST_Extent(ST_Transform(rast::geometry, 4326)) FROM elevation_raster`.
const SW: [f64; 2] = [123.9998611, 33.000244913560735];
const NE: [f64; 2] = [131.9997935265517, 39.000138899999996];

const CLASSIFY_BATCH: usize = 2000;

/// Object key of the single fallback tile every open-ocean page resolves to.
const OCEAN_KEY: &str = "_ocean";

const HAS_LAND_SQL: &str = "
  SELECT t.ord, EXISTS (
    SELECT 1 FROM overture_base_land c
    WHERE c.geom && ST_MakeEnvelope(t.minx, t.miny, t.maxx, t.maxy, 3857)
      AND ST_Intersects(c.geom, ST_MakeEnvelope(t.minx, t.miny, t.maxx, t.maxy, 3857))
  ) AS has_land
  FROM unnest($1::int[], $2::float8[], $3::float8[], $4::float8[], $5::float8[])
    AS t(ord, minx, miny, maxx, maxy)
";

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

    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < argv.len() {
        let next = |i: &mut usize| -> Result<String, String> {
            *i += 1;
            argv.get(*i).cloned().ok_or_else(|| format!("{} wants a value", argv[*i - 1]))
        };
        match argv[i].as_str() {
            "--out" => args.out = PathBuf::from(next(&mut i)?),
            "--concurrency" => args.concurrency = next(&mut i)?.parse().map_err(|_| "bad --concurrency")?,
            "--limit" => args.limit = Some(next(&mut i)?.parse().map_err(|_| "bad --limit")?),
            "--force" => args.force = true,
            "--reclassify" => args.reclassify = true,
            other => return Err(format!("unknown flag: {other}")),
        }
        i += 1;
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

// ---- candidate grid + classification ----

struct Candidate {
    page_idx: IVec,
    bbox: [f64; 4],
}

fn project_page(lng: f64, lat: f64) -> IVec {
    let mut p = Pvf3 { i: 0, f: 0 };
    epsg4326::project(&mut p, &[lng, lat, 0.0]);
    L3_IDX(p.i)
}

fn build_candidates() -> Vec<Candidate> {
    let page_sw = project_page(SW[0], SW[1]);
    let page_ne = project_page(NE[0], NE[1]);

    let (x1, x2) = (ivec::X(page_sw), ivec::X(page_ne));
    let (y1, y2) = (ivec::Y(page_sw), ivec::Y(page_ne));
    let (px_min, px_max) = (x1.min(x2), x1.max(x2));
    let (py_min, py_max) = (y1.min(y2), y1.max(y2));

    let mut out = Vec::new();
    for px in px_min..=px_max {
        for py in py_min..=py_max {
            let page_idx = ivec::gather(px, py, 0);
            let bb = epsg3857::get_page_bbox(page_idx);
            out.push(Candidate {
                page_idx,
                bbox: [bb[0][0], bb[0][1], bb[1][0], bb[1][1]],
            });
        }
    }
    println!("[bake] page grid x[{px_min}..{px_max}] y[{py_min}..{py_max}] = {} candidate pages", out.len());
    out
}

fn classify_land_pages(client: &mut Client, candidates: &[Candidate]) -> Result<Vec<IVec>, postgres::Error> {
    let mut land: Vec<IVec> = Vec::new();

    for (at, batch) in candidates.chunks(CLASSIFY_BATCH).enumerate() {
        let ord: Vec<i32> = (0..batch.len() as i32).collect();
        let minx: Vec<f64> = batch.iter().map(|c| c.bbox[0]).collect();
        let miny: Vec<f64> = batch.iter().map(|c| c.bbox[1]).collect();
        let maxx: Vec<f64> = batch.iter().map(|c| c.bbox[2]).collect();
        let maxy: Vec<f64> = batch.iter().map(|c| c.bbox[3]).collect();
        let params: [&(dyn ToSql + Sync); 5] = [&ord, &minx, &miny, &maxx, &maxy];

        for row in client.query(HAS_LAND_SQL, &params)? {
            if row.get::<_, bool>("has_land") {
                land.push(batch[row.get::<_, i32>("ord") as usize].page_idx);
            }
        }

        let done = (at * CLASSIFY_BATCH + batch.len()).min(candidates.len());
        println!("[bake/classify] {done}/{} land={}", candidates.len(), land.len());
    }
    Ok(land)
}

// ---- ocean fallback tile ----

/// The source packet an open-ocean page would produce: a flat zero elevation
/// grid, no terrain features, no coastline rows. Synthesised rather than read
/// from the DB so the fallback tile is exactly "nothing here but water",
/// independent of whichever far-offshore page happened to get sampled.
fn empty_source() -> Vec<u8> {
    let ele = vec![0i16; (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize];
    let mut empty: TreeView<'_> = TreeView::new();
    empty.branch(Some(&mut |_| {}));

    let mut root: TreeView<'_> = TreeView::new();
    root.branch(Some(&mut |r| {
        r.i16_array(Some(&ele));
        r.subtree(&empty); // buildings
        r.subtree(&empty); // segments
        r.subtree(&empty); // base
        r.subtree(&empty); // coasts
    }));
    encoder().encode(&root, TreeEncoderOptions::default()).expect("empty source packet")
}

/// Bakes the ocean fallback and asserts it really is page-independent — the
/// client reuses one decoded copy for every ocean page, which is only sound if
/// baking the same empty source at different pages (different bbox, different
/// `Z_SCALE` by latitude) yields identical bytes.
fn bake_ocean_tile(samples: &[IVec]) -> Result<Vec<u8>, String> {
    let src = empty_source();
    let baked: Vec<Vec<u8>> = samples.iter().map(|p| bake_terrain(*p, &src).expect("ocean bake")).collect();

    for (i, tile) in baked.iter().enumerate().skip(1) {
        if tile != &baked[0] {
            return Err(format!(
                "[bake/ocean] ocean tile is NOT page-independent: page {} and {} baked to different bytes ({} vs {}). \
                 The shared-fallback plan does not hold; ocean pages would each need their own tile.",
                ivec::repr(samples[0]),
                ivec::repr(samples[i]),
                baked[0].len(),
                tile.len()
            ));
        }
    }
    Ok(baked.into_iter().next().expect("one sample"))
}

// ---- run ----

#[derive(Default)]
struct Totals {
    baked: u64,
    skipped: u64,
    failed: u64,
    raw: u64,
    gz: u64,
    invalid_ele_shape: u64,
}

fn bake_one(repo: &TerrainRepository, client: &mut Client, page_idx: IVec, path: &Path, stats: &mut SourceStats) -> Result<(usize, usize), String> {
    let raw = fetch_tile(repo, client, page_idx, stats).map_err(|e| e.to_string())?;
    let gz = gzip(raw.as_slice()).map_err(|e| e.to_string())?;

    write_atomic(path, &gz).map_err(|e| e.to_string())?;

    Ok((raw.len(), gz.len()))
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    println!(
        "[bake] out={} concurrency={} limit={} force={}",
        args.out.display(),
        args.concurrency,
        args.limit.map(|n| n.to_string()).unwrap_or_else(|| "none".into()),
        args.force
    );
    fs::create_dir_all(&args.out).map_err(|e| e.to_string())?;

    let url = std::env::var("DB_GEO_URL").map_err(|_| "env:DB_GEO_URL:missing")?;
    let mut client = Client::connect(&url, NoTls).map_err(|e| e.to_string())?;

    let candidates = build_candidates();

    // ---- classify (cached: the scan is deterministic and slow-ish) ----
    let land_cache = args.out.join("../land_pages.json");
    let land: Vec<IVec> = match fs::read_to_string(&land_cache) {
        Ok(raw) if !args.reclassify => {
            let list: Vec<String> = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
            let land: Vec<IVec> = list.iter().map(|s| s.parse().expect("page_idx")).collect();
            println!("[bake/classify] reusing cached list: {} land pages ({})", land.len(), land_cache.display());
            land
        }
        _ => {
            let land = classify_land_pages(&mut client, &candidates).map_err(|e| e.to_string())?;
            let list: Vec<String> = land.iter().map(|p| p.to_string()).collect();
            fs::write(&land_cache, serde_json::to_string(&list).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            println!("[bake/classify] done: land={} ocean={} -> {}", land.len(), candidates.len() - land.len(), land_cache.display());
            land
        }
    };
    if land.is_empty() {
        return Err("no land pages: nothing to bake".to_string());
    }

    // ---- ocean fallback tile ----
    let ocean_path = args.out.join(OCEAN_KEY);
    if args.force || !ocean_path.exists() {
        let ocean_raw = bake_ocean_tile(&[land[0], candidates[0].page_idx, candidates[candidates.len() - 1].page_idx])?;
        let ocean_gz = gzip(ocean_raw.as_slice()).map_err(|e| e.to_string())?;
        fs::write(&ocean_path, &ocean_gz).map_err(|e| e.to_string())?;
        println!(
            "[bake/ocean] wrote {OCEAN_KEY}: raw={} gz={} (page-independent: verified)",
            fmt_bytes(ocean_raw.len() as f64),
            fmt_bytes(ocean_gz.len() as f64)
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
                    let path = args.out.join(page_idx.to_string());

                    if !args.force && path.exists() {
                        totals.lock().expect("totals").skipped += 1;
                        continue;
                    }

                    let before = stats.invalid_ele_shape;
                    let result = bake_one(&repo, &mut client, page_idx, &path, &mut stats);

                    let done = {
                        let mut t = totals.lock().expect("totals");
                        match result {
                            Ok((raw, gz)) => {
                                t.baked += 1;
                                t.raw += raw as u64;
                                t.gz += gz as u64;
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
                            "[bake] {done}/{} baked={} skipped={} failed={} gz={} {rate:.0}ms/tile eta={}",
                            targets.len(),
                            t.baked,
                            t.skipped,
                            t.failed,
                            fmt_bytes(t.gz as f64),
                            fmt_dur(rate * (targets.len() as u64 - done) as f64)
                        );
                    }
                }
            });
        }
    });

    let t = totals.into_inner().expect("totals");
    let elapsed = started.elapsed().as_secs_f64() * 1000.0;
    let avg_gz = if t.baked > 0 { t.gz as f64 / t.baked as f64 } else { 0.0 };

    println!();
    println!("[bake] done in {}: baked={} skipped={} failed={}", fmt_dur(elapsed), t.baked, t.skipped, t.failed);
    println!(
        "[bake] raw={} gz={} (avg {}/tile gzipped, {:.1}% of raw)",
        fmt_bytes(t.raw as f64),
        fmt_bytes(t.gz as f64),
        fmt_bytes(avg_gz),
        t.gz as f64 / t.raw.max(1) as f64 * 100.0
    );
    println!("[bake] projected full set ({} land tiles): {}", land.len(), fmt_bytes(avg_gz * land.len() as f64));

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

    // The run counters above describe one invocation, and this tool resumes —
    // a re-run that skips everything would otherwise write a manifest claiming
    // the tile set is empty. Measure what is actually on disk instead, so the
    // manifest describes the artifact rather than the invocation that last
    // touched it.
    let (objects, gz_bytes) = measure_output(&args.out).map_err(|e| e.to_string())?;

    let manifest = serde_json::json!({
        "version": args.out.file_name().map(|n| n.to_string_lossy().to_string()),
        "updated_at": now_iso8601(),
        // The catalog below says which *data* this was baked from; this says
        // which *code* did the baking.
        "git_sha": git_sha(),
        "bbox_4326": { "sw": SW, "ne": NE },

        // The published tile set, measured from disk.
        "objects": objects,
        "gz_bytes": gz_bytes,
        "candidate_pages": candidates.len(),
        "land_pages": land.len(),
        "ocean_pages": candidates.len() - land.len(),
        "ocean_key": OCEAN_KEY,

        // Every object is stored pre-gzipped: upload with
        //   content-type: application/octet-stream
        //   content-encoding: gzip
        // so fetch() on the client decompresses transparently.
        "content_type": "application/octet-stream",
        "content_encoding": "gzip",

        // This invocation only — a resumed run reports only what it re-baked,
        // so a zero here means "nothing to do", not "nothing wrong".
        "run": {
            "baked": t.baked,
            "skipped": t.skipped,
            "failed": t.failed,
            "gz_bytes": t.gz,
            "flat_elevation_tiles": t.invalid_ele_shape,
        },

        "source": read_source_catalogs(&mut client),
    });

    let manifest_path = args.out.join("../manifest.json");
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    println!("[bake] manifest -> {}", manifest_path.display());

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
