// pxl-geo-bake/bin/bake_tiles.rs

//! Offline pre-bake pass: walks every L3 page inside the geo DB's coverage
//! bbox, skips the ones that are pure open ocean, and bakes the rest into
//! packed tile objects (util.rs's CONTENT_ENCODING) ready to upload to a static object store and be served
//! straight to the client with no API server in the loop.
//!
//! A page is built in three stages, because its two halves change for
//! different reasons (terrain/artifact.rs):
//!
//! - `surface`: each land page's ground and elevation, from the database,
//!   into `<build>/surface/`. The expensive one, run when the source data
//!   changes; resumed by what is on disk.
//! - `lines`: each land page's decks, ramps and bores, from what
//!   `solve:decks` wrote, into `<build>/lines/`. Cheap, and made again whole
//!   every time, so a page that lost its lines keeps none: run after every
//!   solve.
//! - `assemble`: each leaf made of its surface and its lines, under
//!   `<out>/0/`, and every level above folded from the one below it
//!   (pyramid.rs) into `<out>/<level>/` up to TOP_LEVEL — with no database
//!   in the loop. Always whole.
//!
//! So a change to how decks are solved is `solve:decks`, then `--stage
//! lines --stage assemble`, and no surface is baked again. With no
//! `--stage`, all three run.
//!
//! Land/ocean classification is a coarse pre-filter, cheaper and separate from
//! the bake path's own fine-grained coastline carving. It checks
//! `overture_base_land` directly for existence, not the full carve; a page with
//! zero touching land rows is open ocean and gets served by the client's single
//! shared fallback tile instead.
//!
//! A page is the same bytes the dev tile server makes of it, which makes and
//! assembles both halves in one go (terrain/leaf.rs `bake_leaf`).
//!
//! Usage (from the repo root):
//!   cargo run --profile server -p pxl-geo-bake --bin bake_tiles -- [flags]
//!
//! Flags:
//!   --stage <s>          surface, lines or assemble; repeatable (default: all three)
//!   --out <dir>          published pages (default ./out/terrain/<published version>)
//!   --build <dir>        the stages' artifacts (default ./out/build/terrain)
//!   --concurrency <n>    parallel pages, one connection each (default 8)
//!   --limit <n>          stop after n land pages (smoke test)
//!   --force              bake surfaces that already exist on disk again
//!   --reclassify         ignore a cached land_pages.json and re-run the scan

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use postgres::{Client, NoTls};
use pxl_spec::bake::format::{encode_terrain, Terrain};
use pxl_spec::bake::published::PUBLISHED_VERSION;
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pyramid::LevelPage;

use pxl_geo_bake::cli::Flags;
use pxl_geo_bake::elevation::Raster;
use pxl_geo_bake::page::walk::{land_pages, write_land_pages};
use pxl_geo_bake::pyramid::{build, write_manifest, Layer, PageSet};
use pxl_geo_bake::terrain::artifact::{assemble, decode_lines, encode_lines, Surface};
use pxl_geo_bake::terrain::leaf::{bake_ocean, has_land, ocean_samples, Coverage, NE, OCEAN_KEY, SW};
use pxl_geo_bake::terrain::repository::TerrainRepository;
use pxl_geo_bake::terrain::source::{fetch_lines, fetch_surface, SourceStats};
use pxl_geo_bake::util::{fmt_bytes, fmt_dur, git_sha, pack, refuse_mixed, unpack, Level};

const CLASSIFY_BATCH: usize = 2000;

// ---- args ----

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Surface,
    Lines,
    Assemble,
}

struct Args {
    stages: Vec<Stage>,
    out: PathBuf,
    build: PathBuf,
    concurrency: usize,
    limit: Option<usize>,
    force: bool,
    reclassify: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        stages: Vec::new(),
        out: PathBuf::from(format!("./out/terrain/{}", PUBLISHED_VERSION.terrain)),
        build: PathBuf::from("./out/build/terrain"),
        concurrency: 8,
        limit: None,
        force: false,
        reclassify: false,
    };

    let mut flags = Flags::from_env();
    while let Some(flag) = flags.arg() {
        match flag.as_str() {
            "--stage" => args.stages.push(match flags.value()?.as_str() {
                "surface" => Stage::Surface,
                "lines" => Stage::Lines,
                "assemble" => Stage::Assemble,
                other => return Err(format!("unknown stage: {other}")),
            }),
            "--out" => args.out = PathBuf::from(flags.value()?),
            "--build" => args.build = PathBuf::from(flags.value()?),
            "--concurrency" => args.concurrency = flags.parse()?,
            "--limit" => args.limit = Some(flags.parse()?),
            "--force" => args.force = true,
            "--reclassify" => args.reclassify = true,
            other => return Err(format!("unknown flag: {other}")),
        }
    }
    if args.stages.is_empty() {
        args.stages = vec![Stage::Surface, Stage::Lines, Stage::Assemble];
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

/// What one stage did, over the pages it was given.
#[derive(Default)]
struct Totals {
    done: u64,
    skipped: u64,
    failed: u64,
    bytes: u64,
    flat_ele: u64,
}

/// Runs `each` over `pages` on `workers` threads, each with what `open`
/// gives it for the whole run, and reports as it goes. `each` says how many
/// bytes it wrote, or that it skipped the page.
fn over_pages<W>(what: &str, pages: &[IVec], workers: usize, open: impl Fn() -> Result<W, String> + Sync, each: impl Fn(&mut W, IVec, &mut Totals) -> Result<Option<u64>, String> + Sync) -> Totals {
    let next = AtomicUsize::new(0);
    let totals = Mutex::new(Totals::default());
    let started = Instant::now();
    std::thread::scope(|scope| {
        for _ in 0..workers.max(1) {
            scope.spawn(|| {
                let mut w = match open() {
                    Ok(w) => w,
                    Err(err) => return eprintln!("[bake/{what}] worker failed to start: {err}"),
                };
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(&page) = pages.get(i) else { break };
                    let mut local = Totals::default();
                    let result = each(&mut w, page, &mut local);
                    let mut t = totals.lock().expect("totals");
                    t.flat_ele += local.flat_ele;
                    match result {
                        Ok(Some(bytes)) => {
                            t.done += 1;
                            t.bytes += bytes;
                        }
                        Ok(None) => t.skipped += 1,
                        Err(err) => {
                            t.failed += 1;
                            eprintln!("[bake/{what}/{}] failed: {err}", ivec::repr(page));
                        }
                    }
                    let n = t.done + t.skipped + t.failed;
                    if n.is_multiple_of(1000) {
                        let rate = started.elapsed().as_secs_f64() * 1000.0 / t.done.max(1) as f64;
                        println!(
                            "[bake/{what}] {n}/{} done={} skipped={} failed={} {} {rate:.0}ms/page eta={}",
                            pages.len(),
                            t.done,
                            t.skipped,
                            t.failed,
                            fmt_bytes(t.bytes as f64),
                            fmt_dur(rate * (pages.len() as u64 - n) as f64)
                        );
                    }
                }
            });
        }
    });
    let t = totals.into_inner().expect("totals");
    println!(
        "[bake/{what}] done in {}: done={} skipped={} failed={} {}",
        fmt_dur(started.elapsed().as_secs_f64() * 1000.0),
        t.done,
        t.skipped,
        t.failed,
        fmt_bytes(t.bytes as f64)
    );
    t
}

/// A worker's connection and the statements it asks with.
struct Db {
    client: Client,
    repo: TerrainRepository,
}

fn connect(url: &str, raster: &Arc<Raster>) -> Result<Db, String> {
    let mut client = Client::connect(url, NoTls).map_err(|e| e.to_string())?;
    let repo = TerrainRepository::prepare(&mut client, raster.clone()).map_err(|e| e.to_string())?;
    Ok(Db { client, repo })
}

fn write_json(path: &Path, value: &serde_json::Value) -> Result<(), String> {
    fs::write(path, serde_json::to_string_pretty(value).map_err(|e| e.to_string())?).map_err(|e| format!("{}: {e}", path.display()))
}

fn read_json(path: &Path) -> serde_json::Value {
    fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(serde_json::Value::Null)
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    let surfaces = PageSet::new(args.build.join("surface"));
    let lines = PageSet::new(args.build.join("lines"));
    let coverage = Coverage::of_source();
    // Beside the published versions, which share it.
    let land_cache = args.out.parent().unwrap_or(Path::new(".")).join("land_pages.json");
    // The database is only asked by the stages that read it.
    let url = std::env::var("DB_GEO_URL").map_err(|_| "env:DB_GEO_URL:missing".to_string());

    if args.stages.contains(&Stage::Surface) {
        let url = url.as_deref().map_err(Clone::clone)?;
        let mut client = Client::connect(url, NoTls).map_err(|e| e.to_string())?;
        // One raster for every worker, so a tile pages share is read once.
        let raster = Arc::new(Raster::open(&mut client).map_err(|e| e.to_string())?);
        let candidates = coverage.pages();
        println!(
            "[bake] page grid x[{}..{}] y[{}..{}] = {} candidate pages",
            coverage.x.0,
            coverage.x.1,
            coverage.y.0,
            coverage.y.1,
            candidates.len()
        );

        // The scan is deterministic and slow-ish, so it is kept; a list
        // classified at another page size is not reusable, and this is the
        // one stage that can classify again, so a refused cache is redone.
        let cached = if args.reclassify { Err("--reclassify".to_string()) } else { land_pages(&land_cache) };
        let land: Vec<IVec> = match cached {
            Ok(land) => {
                println!("[bake/classify] reusing cached list: {} land pages ({})", land.len(), land_cache.display());
                land
            }
            Err(why) => {
                println!("[bake/classify] classifying: {why}");
                let land = classify_land_pages(&mut client, &candidates).map_err(|e| e.to_string())?;
                fs::create_dir_all(land_cache.parent().unwrap_or(Path::new("."))).map_err(|e| e.to_string())?;
                write_land_pages(&land_cache, &land)?;
                println!("[bake/classify] done: land={} ocean={} -> {}", land.len(), candidates.len() - land.len(), land_cache.display());
                land
            }
        };
        let targets = &land[..args.limit.map_or(land.len(), |n| n.min(land.len()))];
        fs::create_dir_all(surfaces.level_dir(0)).map_err(|e| e.to_string())?;
        if !args.force {
            refuse_mixed(&surfaces.level_dir(0))?;
        }
        let t = over_pages(
            "surface",
            targets,
            args.concurrency,
            || connect(url, &raster),
            |db, page, t| {
                let at = LevelPage::new(0, page);
                // Resumed: a surface on disk stands if it is current, and one
                // written in another layout is baked again.
                let path = surfaces.path(at);
                if !args.force && path.exists() && fs::read(&path).ok().and_then(|p| unpack(&p).ok()).is_some_and(|raw| Surface::is_current(&raw)) {
                    return Ok(None);
                }
                let mut stats = SourceStats::default();
                let surface = fetch_surface(&db.repo, &mut db.client, page, &mut stats).map_err(|e| e.to_string())?;
                t.flat_ele += stats.flat_ele;
                let packed = pack(&surface.encode().map_err(|e| format!("{e:?}"))?, Level::Stored).map_err(|e| e.to_string())?;
                surfaces.write(at, &packed)?;
                Ok(Some(packed.len() as u64))
            },
        );
        // A degraded page still writes successfully, so it never shows up in
        // `failed` — surface it here rather than leaving it buried in
        // per-page warnings that nobody greps for after a long run.
        if t.flat_ele > 0 {
            eprintln!(
                "[bake/surface] WARNING: {} pages are out of the elevation raster's reach and were baked FLAT (zero-filled elevation).",
                t.flat_ele
            );
        }
        write_json(
            &args.build.join("surface.json"),
            &serde_json::json!({
                "git_sha": git_sha(),
                "land_pages": land.len(),
                "flat_elevation_pages": t.flat_ele,
                "source": read_source_catalogs(&mut client),
            }),
        )?;
    }

    let land = land_pages(&land_cache)?;
    let targets = &land[..args.limit.map_or(land.len(), |n| n.min(land.len()))];

    if args.stages.contains(&Stage::Lines) {
        let url = url.as_deref().map_err(Clone::clone)?;
        let mut client = Client::connect(url, NoTls).map_err(|e| e.to_string())?;
        let raster = Arc::new(Raster::open(&mut client).map_err(|e| e.to_string())?);
        // Whole every time: a page whose lines the solve took away keeps none.
        if lines.root.exists() {
            fs::remove_dir_all(&lines.root).map_err(|e| e.to_string())?;
        }
        fs::create_dir_all(lines.level_dir(0)).map_err(|e| e.to_string())?;
        over_pages(
            "lines",
            targets,
            args.concurrency,
            || connect(url, &raster),
            |db, page, _| {
                let decks = fetch_lines(&db.repo, &mut db.client, page).map_err(|e| e.to_string())?;
                if decks.is_empty() {
                    return Ok(None);
                }
                let packed = pack(&encode_lines(&decks).map_err(|e| format!("{e:?}"))?, Level::Stored).map_err(|e| e.to_string())?;
                lines.write(LevelPage::new(0, page), &packed)?;
                Ok(Some(packed.len() as u64))
            },
        );
        write_json(&args.build.join("lines.json"), &serde_json::json!({ "git_sha": git_sha() }))?;
    }

    if args.stages.contains(&Stage::Assemble) {
        let tiles = PageSet::new(&args.out);
        // Whole every time, as the levels above always were: a leaf is a
        // function of its two artifacts, and none is left from another run.
        if tiles.root.exists() {
            fs::remove_dir_all(&tiles.root).map_err(|e| e.to_string())?;
        }
        fs::create_dir_all(tiles.level_dir(0)).map_err(|e| e.to_string())?;

        let ocean_raw = bake_ocean(&ocean_samples(&coverage))?;
        let ocean_packed = pack(ocean_raw.as_slice(), Level::Stored).map_err(|e| e.to_string())?;
        fs::write(tiles.root.join(OCEAN_KEY), &ocean_packed).map_err(|e| e.to_string())?;
        println!("[bake/ocean] wrote {OCEAN_KEY}: {} (page-independent: verified)", fmt_bytes(ocean_packed.len() as f64));

        let read = |set: &PageSet, page: IVec| -> Result<Option<Vec<u8>>, String> {
            let path = set.path(LevelPage::new(0, page));
            if !path.exists() {
                return Ok(None);
            }
            unpack(&fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?).map(Some).map_err(|e| e.to_string())
        };
        // An artifact written in another layout is refused before any page
        // is assembled, not a page at a time after.
        if let Some(&first) = targets.first() {
            Surface::decode(&read(&surfaces, first)?.ok_or("no surface: run --stage surface")?)?;
        }
        if let Some(raw) = lines.pages(0)?.first().map(|p| read(&lines, p.page)).transpose()?.flatten() {
            decode_lines(&raw)?;
        }
        let t = over_pages(
            "assemble",
            targets,
            args.concurrency,
            || Ok(()),
            |_, page, _| {
                let surface = read(&surfaces, page)?.ok_or("no surface: run --stage surface")?;
                let surface = Surface::decode(&surface)?;
                let decks = match read(&lines, page)? {
                    Some(raw) => decode_lines(&raw)?,
                    None => Default::default(),
                };
                let raw = encode_terrain(&assemble(surface, decks)).map_err(|e| format!("{e:?}"))?;
                let packed = pack(&raw, Level::Stored).map_err(|e| e.to_string())?;
                tiles.write(LevelPage::new(0, page), &packed)?;
                Ok(Some(packed.len() as u64))
            },
        );
        if t.failed > 0 {
            return Err(format!("{} pages failed to assemble", t.failed));
        }

        let ocean = Terrain::decode(&unpack(&ocean_packed).map_err(|e| e.to_string())?)?;
        let top = build(&tiles, &ocean, args.concurrency)?;
        let surface = read_json(&args.build.join("surface.json"));
        write_manifest(
            &tiles,
            top,
            serde_json::json!({
                "bbox_4326": { "sw": SW, "ne": NE },
                "candidate_pages": coverage.pages().len(),
                "land_pages": land.len(),
                "ocean_key": OCEAN_KEY,
                "flat_elevation_pages": surface["flat_elevation_pages"],
                "source": surface["source"],
                "built": {
                    "surface_git_sha": surface["git_sha"],
                    "lines_git_sha": read_json(&args.build.join("lines.json"))["git_sha"],
                },
            }),
        )?;
    }
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
