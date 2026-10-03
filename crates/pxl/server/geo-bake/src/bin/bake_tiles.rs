// pxl-geo-bake/bin/bake_tiles.rs

//! The terrain bake, in three stages: `surface` (ground and elevation from the
//! DB, resumable), `lines` (decks and bores from `solve:decks`, whole every
//! time) and `assemble` (leaves and the levels above, no DB). After a deck
//! solve change, run `--stage lines --stage assemble`.
//!
//!   cargo run --profile server -p pxl-geo-bake --bin bake_tiles -- [flags]
//!
//!   --stage <s>          surface, lines or assemble; repeatable (default: all three)
//!   --out <dir>          published pages (default ./out/terrain/<published version>)
//!   --build <dir>        the stages' artifacts (default ./out/build/terrain)
//!   --concurrency <n>    parallel pages, one connection each (default 8)
//!   --limit <n>          stop after n land pages (smoke test)
//!   --force              bake surfaces that already exist on disk again
//!   --reclassify         classify again though land_pages.json is current

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use postgres::Client;
use pxl_spec::bake::format::{encode_terrain, Terrain};
use pxl_spec::bake::published::PUBLISHED_VERSION;
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pyramid::LevelPage;

use pxl_geo_bake::cli::Flags;
use pxl_geo_bake::coverage::{self, Coverage, Overture};
use pxl_geo_bake::elevation::Raster;
use pxl_geo_bake::pyramid::{build, write_manifest, Layer, PageSet};
use pxl_geo_bake::terrain::artifact::{assemble, decode_lines, encode_lines, Surface};
use pxl_geo_bake::terrain::leaf::{self, bake_ocean, has_ground, land_pages, ocean_samples, write_land_pages, OCEAN_KEY};
use pxl_geo_bake::terrain::repository::TerrainRepository;
use pxl_geo_bake::terrain::source::{fetch_lines, fetch_surface, SourceStats};
use pxl_geo_bake::util::{fmt_bytes, fmt_dur, git_sha, pack, refuse_mixed, unpack, Level};

const CLASSIFY_BATCH: usize = 2000;

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

/// What the geo DB held when a version was baked; the version name says nothing of it.
fn read_source_catalogs(client: &mut Client, source: &Overture) -> serde_json::Value {
    let sql = "SELECT region_code, data_date::text AS data_date, file_name, status FROM elevation_catalog ORDER BY file_name";
    let elevation: Vec<serde_json::Value> = match client.query(sql, &[]) {
        Ok(rows) => rows
            .iter()
            .map(|r| {
                serde_json::json!({
                    "region_code": r.get::<_, String>("region_code"),
                    "data_date": r.get::<_, Option<String>>("data_date"),
                    "file_name": r.get::<_, String>("file_name"),
                    "status": r.get::<_, String>("status"),
                })
            })
            .collect(),
        Err(err) => {
            eprintln!("[bake/provenance] could not read elevation_catalog: {err}");
            Vec::new()
        }
    };
    serde_json::json!({ "overture": source.manifest(), "elevation_catalog": elevation })
}

fn classify_land_pages(client: &mut Client, candidates: &[IVec]) -> Result<Vec<IVec>, postgres::Error> {
    let mut land: Vec<IVec> = Vec::new();
    for (at, batch) in candidates.chunks(CLASSIFY_BATCH).enumerate() {
        for (&page, touches) in batch.iter().zip(has_ground(client, batch)?) {
            if touches {
                land.push(page);
            }
        }
        let done = (at * CLASSIFY_BATCH + batch.len()).min(candidates.len());
        println!("[bake/classify] {done}/{} land={}", candidates.len(), land.len());
    }
    Ok(land)
}

#[derive(Default)]
struct Totals {
    done: u64,
    skipped: u64,
    failed: u64,
    bytes: u64,
    flat_ele: u64,
}

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

struct Db {
    client: Client,
    repo: TerrainRepository,
}

fn connect(source: &Overture, raster: &Arc<Raster>) -> Result<Db, String> {
    let mut client = source.connect()?;
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
    let coverage = Coverage::of_source()?;
    let land_cache = args.out.parent().unwrap_or(Path::new(".")).join("land_pages.json");
    let (source, mut checked) = match args.stages.iter().any(|&s| s != Stage::Assemble) {
        true => Overture::reading(&[&leaf::GROUND_READS, &pxl_geo_bake::terrain::repository::READS, &pxl_geo_bake::terrain::repository::DECK_READS]).map(|(s, c)| (Some(s), Some(c)))?,
        false => (None, None),
    };

    if args.stages.contains(&Stage::Surface) {
        let source = source.as_ref().expect("opened for a stage that reads the DB");
        let mut client = match checked.take() {
            Some(client) => client,
            None => source.connect()?,
        };
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

        let cached = if args.reclassify { Err("--reclassify".to_string()) } else { land_pages(&land_cache, &coverage, Some(&source.release)) };
        let land: Vec<IVec> = match cached {
            Ok(land) => {
                println!("[bake/classify] reusing cached list: {} land pages ({})", land.len(), land_cache.display());
                land
            }
            Err(why) => {
                println!("[bake/classify] classifying: {why}");
                let land = classify_land_pages(&mut client, &candidates).map_err(|e| e.to_string())?;
                fs::create_dir_all(land_cache.parent().unwrap_or(Path::new("."))).map_err(|e| e.to_string())?;
                write_land_pages(&land_cache, &coverage, &source.release, &land)?;
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
            || connect(source, &raster),
            |db, page, t| {
                let at = LevelPage::new(0, page);
                if !args.force && surfaces.path(at).exists() {
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
        // A degraded page never shows up in `failed`, so report it here.
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
                "source": read_source_catalogs(&mut client, source),
            }),
        )?;
    }

    let land = land_pages(&land_cache, &coverage, source.as_ref().map(|s| s.release.as_str()))?;
    let targets = &land[..args.limit.map_or(land.len(), |n| n.min(land.len()))];

    if args.stages.contains(&Stage::Lines) {
        let source = source.as_ref().expect("opened for a stage that reads the DB");
        let mut client = match checked.take() {
            Some(client) => client,
            None => source.connect()?,
        };
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
            || connect(source, &raster),
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
        let t = over_pages(
            "assemble",
            targets,
            args.concurrency,
            || Ok(()),
            |_, page, _| {
                let surface = read(&surfaces, page)?.ok_or("no surface: run --stage surface")?;
                let surface = Surface::decode(&surface).map_err(|e| format!("{e:?}"))?;
                let decks = match read(&lines, page)? {
                    Some(raw) => decode_lines(&raw).map_err(|e| format!("{e:?}"))?,
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
                "bbox_4326": coverage::source_bbox()?,
                "ingest_bbox": coverage.ingest_bbox(),
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
