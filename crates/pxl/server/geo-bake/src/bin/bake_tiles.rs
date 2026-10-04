// pxl-geo-bake/bin/bake_tiles.rs

//! The terrain bake, in two stages: `leaf` (each page as the dev server bakes
//! it on demand, terrain/leaf.rs `bake_leaf`; resumable) and `assemble` (the
//! leaves and the zooms folded above them, no DB).
//!
//!   cargo run --profile server -p pxl-geo-bake --bin bake_tiles -- [flags]
//!
//!   --stage <s>          leaf or assemble; repeatable (default: both)
//!   --out <dir>          published pages (default ./out/terrain/v<format>.<UTC day>.<n>, the day's next)
//!   --build <dir>        the stages' artifacts (default ./out/build/terrain)
//!   --concurrency <n>    parallel pages, one connection each (default 8)
//!   --limit <n>          stop after n land pages (smoke test)
//!   --force              bake leaves that already exist on disk again, and solve every structure again
//!   --reclassify         classify again though land_pages.json is current
//!   --region <quadkey>   bake again only the land pages under it, repeatable; assemble then
//!                        links --base whole and folds again only what those pages reach
//!   --base <dir>         the version a region bake starts from (default the published one)
//!
//! A region bake gives its pages what a full bake of the same code gives them;
//! the pages outside keep the base's. So a region must hold everything the
//! change it is baked for reaches: a structure it solves again that runs out of
//! it meets the base's solve of it at the region's edge.

use std::collections::HashSet;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use postgres::Client;
use pxl_spec::bake::format::{encode_terrain, Terrain, SNAPSHOT_KEY};
use pxl_spec::bake::published::{FORMAT, PUBLISHED_VERSION};
use pxl_spec::bake::quadkey::{page_name, page_of_name};
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pyramid::{Page, GROUND_Z, TOP_Z};

use pxl_geo_bake::cli::Flags;
use pxl_geo_bake::coverage::{self, Coverage, Overture};
use pxl_geo_bake::deck;
use pxl_geo_bake::elevation::Raster;
use pxl_geo_bake::pyramid::{build, refold, write_manifest, Layer, PageSet};
use pxl_geo_bake::terrain::leaf::{self, bake_leaf, bake_ocean, has_ground, land_pages, ocean_samples, write_land_pages, OCEAN_KEY};
use pxl_geo_bake::terrain::repository::TerrainRepository;
use pxl_geo_bake::util::{fmt_bytes, fmt_dur, git_sha, next_version, pack, refuse_mixed, unpack, Level};

const CLASSIFY_BATCH: usize = 2000;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Leaf,
    Assemble,
}

struct Args {
    stages: Vec<Stage>,
    out: Option<PathBuf>,
    build: PathBuf,
    concurrency: usize,
    limit: Option<usize>,
    force: bool,
    reclassify: bool,
    regions: Vec<Page>,
    base: Option<PathBuf>,
}

impl Args {
    fn in_region(&self, page: IVec) -> bool {
        self.regions.iter().any(|r| Page::over(r.z, page) == *r)
    }
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        stages: Vec::new(),
        out: None,
        build: PathBuf::from("./out/build/terrain"),
        concurrency: 8,
        limit: None,
        force: false,
        reclassify: false,
        regions: Vec::new(),
        base: None,
    };

    let mut flags = Flags::from_env();
    while let Some(flag) = flags.arg() {
        match flag.as_str() {
            "--stage" => args.stages.push(match flags.value()?.as_str() {
                "leaf" => Stage::Leaf,
                "assemble" => Stage::Assemble,
                other => return Err(format!("unknown stage: {other}")),
            }),
            "--out" => args.out = Some(PathBuf::from(flags.value()?)),
            "--build" => args.build = PathBuf::from(flags.value()?),
            "--concurrency" => args.concurrency = flags.parse()?,
            "--limit" => args.limit = Some(flags.parse()?),
            "--force" => args.force = true,
            "--reclassify" => args.reclassify = true,
            "--region" => {
                let key = flags.value()?;
                args.regions.push(page_of_name(&key).ok_or(format!("--region {key}: not a page's quadkey"))?);
            }
            "--base" => args.base = Some(PathBuf::from(flags.value()?)),
            other => return Err(format!("unknown flag: {other}")),
        }
    }
    if args.stages.is_empty() {
        args.stages = vec![Stage::Leaf, Stage::Assemble];
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

fn over_pages<W>(what: &str, pages: &[IVec], workers: usize, open: impl Fn() -> Result<W, String> + Sync, each: impl Fn(&mut W, IVec) -> Result<Option<u64>, String> + Sync) -> Totals {
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
                    let result = each(&mut w, page);
                    let mut t = totals.lock().expect("totals");
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

/// A band's pages row by row, each row the way the last one ended.
fn zigzag(band: &[IVec]) -> Vec<IVec> {
    let mut out = Vec::with_capacity(band.len());
    for (k, row) in band.chunk_by(|a, b| ivec::Y(*a) == ivec::Y(*b)).enumerate() {
        if k % 2 == 0 {
            out.extend(row.iter().copied());
        } else {
            out.extend(row.iter().rev().copied());
        }
    }
    out
}

fn write_json(path: &Path, value: &serde_json::Value) -> Result<(), String> {
    fs::write(path, serde_json::to_string_pretty(value).map_err(|e| e.to_string())?).map_err(|e| format!("{}: {e}", path.display()))
}

fn read_json(path: &Path) -> serde_json::Value {
    fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(serde_json::Value::Null)
}

/// Every file under `from` hard-linked under `to`, directories made.
fn link_tree(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|e| format!("{}: {e}", to.display()))?;
    for entry in fs::read_dir(from).map_err(|e| format!("{}: {e}", from.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let (src, dst) = (entry.path(), to.join(entry.file_name()));
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            link_tree(&src, &dst)?;
        } else {
            fs::hard_link(&src, &dst).map_err(|e| format!("{}: {e}", src.display()))?;
        }
    }
    Ok(())
}

/// What `tiles` writes and removes over the full version `full`, by quadkey:
/// a page linked from it is the same file, and any other one is checked by content.
fn snapshot_over(full: &PageSet, tiles: &PageSet) -> Result<(Vec<String>, Vec<String>), String> {
    let (mut written, mut removed) = (Vec::new(), Vec::new());
    for z in TOP_Z..=GROUND_Z {
        let here: HashSet<Page> = tiles.pages(z)?.into_iter().collect();
        for &p in &here {
            let (a, b) = (full.path(p), tiles.path(p));
            let same = match (fs::metadata(&a), fs::metadata(&b)) {
                (Ok(x), Ok(y)) => (x.dev(), x.ino()) == (y.dev(), y.ino()) || full.content(p)? == tiles.content(p)?,
                _ => false,
            };
            if !same {
                written.push(page_name(p).to_string());
            }
        }
        removed.extend(full.pages(z)?.into_iter().filter(|p| !here.contains(p)).map(|p| page_name(p).to_string()));
    }
    Ok((written, removed))
}

/// A region over a base: the base linked whole, the region's leaves in its
/// leaves' place where they differ, and what those reach folded again.
fn assemble_region(args: &Args, leaves: &PageSet, land: &[IVec], leaf: &serde_json::Value, built: Option<&str>) -> Result<(), String> {
    let base = PageSet::new(args.base.clone().unwrap_or_else(|| Path::new("./out/terrain").join(PUBLISHED_VERSION.terrain)));
    if !base.root.is_dir() {
        return Err(format!("{}: no base to bake a region over; name one with --base", base.root.display()));
    }
    let tiles = PageSet::new(args.out.clone().unwrap_or_else(|| next_version("./out/terrain", FORMAT.terrain)));
    if tiles.root.exists() {
        fs::remove_dir_all(&tiles.root).map_err(|e| e.to_string())?;
    }
    link_tree(&base.root, &tiles.root)?;

    let region_land: HashSet<IVec> = land.iter().copied().filter(|&p| args.in_region(p)).collect();
    let mut changed = HashSet::new();
    for at in base.pages(GROUND_Z)? {
        if args.in_region(at.page) && !region_land.contains(&at.page) {
            tiles.remove(at)?;
            changed.insert(at);
        }
    }
    for &page in &region_land {
        let at = Page::new(GROUND_Z, page);
        let fresh = leaves.content(at)?.ok_or_else(|| format!("{}: run --stage leaf with the region", leaves.path(at).display()))?;
        if base.content(at)?.as_deref() == Some(fresh.as_slice()) {
            continue;
        }
        tiles.remove(at)?;
        fs::hard_link(leaves.path(at), tiles.path(at)).map_err(|e| format!("{}: {e}", leaves.path(at).display()))?;
        changed.insert(at);
    }
    let ocean_packed = fs::read(tiles.root.join(OCEAN_KEY)).map_err(|e| format!("{}: {e}", tiles.root.display()))?;
    let ocean = Terrain::decode(&unpack(&ocean_packed).map_err(|e| e.to_string())?)?;
    let leaves_changed = changed.len();
    let folded = refold(&tiles, changed, &ocean)?;
    println!("[bake/region] {leaves_changed} leaves changed, {folded} pages above folded again -> {}", tiles.root.display());

    // Uploaded, the version is its changes over one full version, never over
    // another partial one: a base that is partial lends its own full base, and
    // its changes come along as this version's.
    let base_snapshot = read_json(&base.root.join(SNAPSHOT_KEY));
    let full = PageSet::new(match base_snapshot["base"].as_str() {
        Some(b) => base.root.parent().unwrap_or(Path::new(".")).join(b),
        None => base.root.clone(),
    });
    if !full.root.is_dir() {
        return Err(format!("{}: the full version under the base is not here", full.root.display()));
    }
    let (written, removed) = snapshot_over(&full, &tiles)?;
    let snapshot = tiles.root.join(SNAPSHOT_KEY);
    // Linked from a partial base: written anew, never through the link.
    let _ = fs::remove_file(&snapshot);
    let full_name = full.root.file_name().map(|n| n.to_string_lossy().to_string());
    write_json(&snapshot, &serde_json::json!({ "base": full_name, "written": written, "removed": removed }))?;
    println!("[bake/region] over {}: {} written, {} removed", full.root.display(), written.len(), removed.len());

    let top = (TOP_Z..GROUND_Z).rev().take_while(|&z| tiles.zoom_dir(z).exists()).last().unwrap_or(GROUND_Z);
    let base_name = base.root.file_name().map(|n| n.to_string_lossy().to_string());
    // The kind's one manifest describes the last bake, which is the base only if it says so.
    let mut fields = read_json(&base.root.join("../manifest.json"));
    if fields["version"].as_str() != base_name.as_deref() {
        fields = serde_json::json!({});
    }
    let base_release = fields["release"].clone();
    if let Some(f) = fields.as_object_mut() {
        for measured in ["version", "updated_at", "git_sha", "content_encoding", "content_type", "objects", "bytes", "zooms"] {
            f.remove(measured);
        }
        f.insert("base".into(), serde_json::json!({ "version": base_name, "release": base_release }));
        f.insert(
            "regions".into(),
            serde_json::json!({ "quadkeys": leaf["regions"], "release": leaf["release"], "leaf_git_sha": leaf["git_sha"] }),
        );
    }
    write_manifest(&tiles, top, built, fields)
}

fn run() -> Result<(), String> {
    let built = git_sha();
    let args = parse_args()?;
    let leaves = PageSet::new(args.build.join("leaf"));
    let coverage = Coverage::of_source()?;
    let land_cache = args.out.as_deref().and_then(Path::parent).unwrap_or(Path::new("./out/terrain")).join("land_pages.json");
    let (source, mut checked) = match args.stages.iter().any(|&s| s != Stage::Assemble) {
        true => Overture::reading(&leaf::READS).map(|(s, c)| (Some(s), Some(c)))?,
        false => (None, None),
    };

    if args.stages.contains(&Stage::Leaf) {
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

        let cached = if args.reclassify {
            Err("--reclassify".to_string())
        } else {
            land_pages(&land_cache, &coverage, Some(&source.release))
        };
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
        let targets: Vec<IVec> = match args.regions.is_empty() {
            true => land[..args.limit.map_or(land.len(), |n| n.min(land.len()))].to_vec(),
            false => land.iter().copied().filter(|&p| args.in_region(p)).collect(),
        };
        // A region's pages are baked again whatever is on disk, by the code that changed.
        let fresh = args.force || !args.regions.is_empty();
        // solved structures, kept for the pages they cross; what a fresh bake solves again
        let structures = args.build.join("structure");
        if fresh {
            match fs::remove_dir_all(&structures) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.to_string()),
                _ => {}
            }
        }
        fs::create_dir_all(leaves.zoom_dir(GROUND_Z)).map_err(|e| e.to_string())?;
        if !fresh {
            refuse_mixed(&leaves.zoom_dir(GROUND_Z))?;
        }
        // Each worker walks its own band of rows, zigzag, so a page's eight
        // around it were mostly read for the page before.
        let mut rows: Vec<IVec> = targets.to_vec();
        rows.sort_unstable_by_key(|&p| (ivec::Y(p), ivec::X(p)));
        let band = rows.len().div_ceil(args.concurrency.max(1)).max(1);
        let walks: Vec<Vec<IVec>> = rows.chunks(band).map(zigzag).collect();
        let totals = Mutex::new(Totals::default());
        std::thread::scope(|scope| {
            let all = rows.len();
            for walk in &walks {
                let (totals, leaves, raster, structures) = (&totals, &leaves, &raster, &structures);
                scope.spawn(move || {
                    let mut db = match connect(source, raster) {
                        Ok(db) => db,
                        Err(err) => return eprintln!("[bake/leaf] worker failed to start: {err}"),
                    };
                    let mut held = deck::source::Walk::default();
                    for &page in walk {
                        held.keep_around(page);
                        let at = Page::new(GROUND_Z, page);
                        let mut flat = false;
                        let baked = (|| -> Result<Option<u64>, String> {
                            if !fresh && leaves.path(at).exists() {
                                return Ok(None);
                            }
                            let (terrain, off_raster) = bake_leaf(
                                &mut deck::Reader {
                                    client: &mut db.client,
                                    repo: &db.repo,
                                    store: &mut held,
                                    memo: structures,
                                },
                                page,
                            )?;
                            flat = off_raster;
                            let packed = pack(&encode_terrain(&terrain).map_err(|e| format!("{e:?}"))?, Level::Stored).map_err(|e| e.to_string())?;
                            leaves.write(at, &packed)?;
                            Ok(Some(packed.len() as u64))
                        })();
                        let mut t = totals.lock().expect("totals");
                        t.flat_ele += u64::from(flat);
                        match baked {
                            Ok(Some(bytes)) => (t.done, t.bytes) = (t.done + 1, t.bytes + bytes),
                            Ok(None) => t.skipped += 1,
                            Err(err) => {
                                t.failed += 1;
                                eprintln!("[bake/leaf/{}] failed: {err}", ivec::repr(page));
                            }
                        }
                        let n = t.done + t.skipped + t.failed;
                        if n.is_multiple_of(1000) {
                            println!("[bake/leaf] {n}/{all} done={} skipped={} failed={} {}", t.done, t.skipped, t.failed, fmt_bytes(t.bytes as f64));
                        }
                    }
                });
            }
        });
        let t = totals.into_inner().expect("totals");
        println!("[bake/leaf] done: done={} skipped={} failed={} {}", t.done, t.skipped, t.failed, fmt_bytes(t.bytes as f64));
        if t.failed > 0 {
            return Err(format!("{} pages failed", t.failed));
        }
        // A degraded page never shows up in `failed`, so report it here.
        if t.flat_ele > 0 {
            eprintln!(
                "[bake/leaf] WARNING: {} pages are out of the elevation raster's reach and were baked FLAT (zero-filled elevation).",
                t.flat_ele
            );
        }
        write_json(
            &args.build.join("leaf.json"),
            &serde_json::json!({
                "git_sha": built,
                "land_pages": land.len(),
                "release": source.release,
                "regions": args.regions.iter().map(|&r| page_name(r).to_string()).collect::<Vec<_>>(),
                "source": read_source_catalogs(&mut client, source),
            }),
        )?;
    }

    let land = land_pages(&land_cache, &coverage, source.as_ref().map(|s| s.release.as_str()))?;
    let targets = &land[..args.limit.map_or(land.len(), |n| n.min(land.len()))];

    if args.stages.contains(&Stage::Assemble) {
        let leaf = read_json(&args.build.join("leaf.json"));
        let release = leaf["release"].as_str().ok_or("leaf.json names no release: run --stage leaf")?.to_string();
        if !args.regions.is_empty() {
            return assemble_region(&args, &leaves, &land, &leaf, built.as_deref());
        }
        let tiles = PageSet::new(args.out.clone().unwrap_or_else(|| next_version("./out/terrain", FORMAT.terrain)));
        if tiles.root.exists() {
            fs::remove_dir_all(&tiles.root).map_err(|e| e.to_string())?;
        }
        fs::create_dir_all(tiles.zoom_dir(GROUND_Z)).map_err(|e| e.to_string())?;

        let ocean_raw = bake_ocean(&ocean_samples(&coverage))?;
        let ocean_packed = pack(ocean_raw.as_slice(), Level::Stored).map_err(|e| e.to_string())?;
        fs::write(tiles.root.join(OCEAN_KEY), &ocean_packed).map_err(|e| e.to_string())?;
        println!("[bake/ocean] wrote {OCEAN_KEY}: {} (page-independent: verified)", fmt_bytes(ocean_packed.len() as f64));

        let t = over_pages(
            "assemble",
            targets,
            args.concurrency,
            || Ok(()),
            |_, page| {
                let (from, to) = (leaves.path(Page::new(GROUND_Z, page)), tiles.path(Page::new(GROUND_Z, page)));
                fs::hard_link(&from, &to).map_err(|e| format!("{}: {e}: run --stage leaf", from.display()))?;
                Ok(Some(fs::metadata(&to).map_err(|e| e.to_string())?.len()))
            },
        );
        if t.failed > 0 {
            return Err(format!("{} pages failed to assemble", t.failed));
        }

        let ocean = Terrain::decode(&unpack(&ocean_packed).map_err(|e| e.to_string())?)?;
        let top = build(&tiles, &ocean, args.concurrency)?;
        write_manifest(
            &tiles,
            top,
            built.as_deref(),
            serde_json::json!({
                "bbox_4326": coverage::source_bbox()?,
                "ingest_bbox": coverage.ingest_bbox(),
                "candidate_pages": coverage.pages().len(),
                "land_pages": land.len(),
                "ocean_key": OCEAN_KEY,
                "release": release,
                "source": leaf["source"],
                "built": { "leaf_git_sha": leaf["git_sha"] },
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
