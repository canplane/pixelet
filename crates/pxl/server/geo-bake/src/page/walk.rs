// pxl-geo-bake/page/walk.rs

//! The reverse-geocode bake's walk: terrain's own land-page list, one object
//! per group of pages, resumable. The walk is over whatever the bake made of
//! the land list rather than over the list itself.
//!
//! Reusing terrain's land/ocean split rather than a classification of its own
//! is deliberate. "Does this bbox touch any land polygon" is a cheap, purely
//! geometric pre-filter that already rules out every page a label could never
//! fall on; a real "does this group have a named division" pass would cost
//! about what fetching costs, so there is nothing cheaper to filter on than
//! what terrain already computed. A land group with nothing on it still costs
//! one round trip — the `.empty` marker's job is to make that a one-time cost,
//! not to remove it.
//!
//! There is no shared fallback the way terrain's `_ocean` is one: "nothing
//! here" is an empty result rather than a payload some other page's bytes
//! could stand in for, so a missing key is enough on its own and the client
//! reads a 404 as empty directly.
//!
//! The pyramid bakes do not go through this. Terrain's walk is over a
//! candidate grid it classifies itself, and POI's leaves are wherever a place
//! is; both then fold (pyramid.rs), which a flat group set has no use for.

use pxl_spec::bake::quadkey::quadkey_of;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use pxl_spec::space::indexer::L3_SHAMT;
use pxl_spec::space::pvf3::ivec::{self, IVec};

use crate::util::{fmt_bytes, fmt_dur, pack, write_atomic, Level};

/// A local marker saying "already asked, nothing here". Never uploaded — the
/// publish step excludes it.
pub const EMPTY_SUFFIX: &str = ".empty";

pub struct Args {
    pub out: PathBuf,
    pub land_cache: PathBuf,
    pub concurrency: usize,
    pub limit: Option<usize>,
    pub force: bool,
}

impl Args {
    /// The five flags both bakes take, with the output directory's default left
    /// to the caller since it names its own kind.
    pub fn parse(default_out: PathBuf) -> Result<Args, String> {
        let mut args = Args {
            out: default_out,
            land_cache: PathBuf::from("./out/terrain/land_pages.json"),
            concurrency: 8,
            limit: None,
            force: false,
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
                "--land-cache" => args.land_cache = PathBuf::from(next(&mut i)?),
                "--concurrency" => args.concurrency = next(&mut i)?.parse().map_err(|_| "bad --concurrency")?,
                "--limit" => args.limit = Some(next(&mut i)?.parse().map_err(|_| "bad --limit")?),
                "--force" => args.force = true,
                other => return Err(format!("unknown flag: {other}")),
            }
            i += 1;
        }
        Ok(args)
    }
}

/// What one target came to, and how many labels it holds.
pub enum Page {
    Object { raw: Vec<u8>, items: u64 },
    Empty,
}

/// One object of the walk: the index it is named by, and the land pages it
/// covers.
pub struct Target {
    pub idx: IVec,
    pub leaves: Vec<IVec>,
}

/// The land pages gathered `shamt` levels up, first-seen order.
pub fn targets_of(land: &[IVec], shamt: u32) -> Vec<Target> {
    let mut out: Vec<Target> = Vec::new();
    let mut at: HashMap<IVec, usize> = HashMap::new();
    for &page in land {
        let idx = ivec::sh(page, -(shamt as i32));
        let i = *at.entry(idx).or_insert_with(|| {
            out.push(Target { idx, leaves: Vec::new() });
            out.len() - 1
        });
        out[i].leaves.push(page);
    }
    out
}

#[derive(Default)]
pub struct Tally {
    pub baked: u64,
    pub empty: u64,
    pub skipped: u64,
    pub failed: u64,
    pub items: u64,
    pub bytes: u64,
}

pub struct Walked {
    pub tally: Tally,
    pub elapsed_ms: f64,
}

/// Terrain's land pages, as its own classification wrote them.
///
/// A page index means a different square of ground at every page size, so the
/// list carries the `L3_SHAMT` it was classified at and is refused at any
/// other. Reused silently, a list from a larger page names a quarter of the
/// land at the wrong places, and a bake of it looks like a bake.
pub fn land_pages(path: &Path) -> Result<Vec<IVec>, String> {
    let reclassify = "run bake_tiles, which writes it as a side effect of its own classification (`--limit 0` does the classification and bakes nothing)";
    let raw = std::fs::read_to_string(path).map_err(|_| format!("{} not found — {reclassify}.", path.display()))?;
    let v: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    let Some(shamt) = v.get("l3_shamt").and_then(|s| s.as_u64()) else {
        return Err(format!("{} does not say the page size it was classified at — {reclassify}.", path.display()));
    };
    if shamt != L3_SHAMT as u64 {
        return Err(format!("{} was classified at L3_SHAMT {shamt}, and a page is now L3_SHAMT {L3_SHAMT} — {reclassify}.", path.display()));
    }
    let list = v.get("pages").and_then(|p| p.as_array()).ok_or_else(|| format!("{} has no page list", path.display()))?;
    list.iter().map(|s| s.as_str().ok_or("a page is not a string")?.parse::<IVec>().map_err(|e| e.to_string())).collect()
}

/// Writes the land list with the page size it was classified at.
pub fn write_land_pages(path: &Path, land: &[IVec]) -> Result<(), String> {
    let pages: Vec<String> = land.iter().map(|p| p.to_string()).collect();
    let v = serde_json::json!({ "l3_shamt": L3_SHAMT, "pages": pages });
    std::fs::write(path, serde_json::to_string(&v).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

/// Walks the pages with one worker per connection, writing what `one` returns.
///
/// `connect` is called once per worker, so a worker owns its connection and its
/// prepared statements for the whole run.
pub fn walk<T>(
    args: &Args,
    tag: &str,
    zoom: u32,
    every: u64,
    targets: &[Target],
    connect: impl Fn() -> Result<T, String> + Sync,
    one: impl Fn(&mut T, &Target) -> Result<Page, String> + Sync,
) -> Result<Walked, String> {
    let next = AtomicUsize::new(0);
    let totals = Mutex::new(Tally::default());
    let started = Instant::now();

    std::thread::scope(|scope| {
        for _ in 0..args.concurrency.max(1) {
            scope.spawn(|| {
                let mut worker = match connect() {
                    Ok(w) => w,
                    Err(err) => return eprintln!("[bake/{tag}] worker: {err}"),
                };

                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(target) = targets.get(i) else { break };
                    let page_idx = target.idx;
                    // The name the bucket serves the object under, written
                    // here so that publishing is a plain sync: unlike terrain,
                    // these bakes keep no level directories, so nothing on
                    // disk wants the index as a name (pxl-spec bake/quadkey.rs).
                    let name = quadkey_of(zoom, page_idx).to_string();
                    let path = args.out.join(&name);
                    let marker = args.out.join(format!("{name}{EMPTY_SUFFIX}"));

                    if !args.force && (path.exists() || marker.exists()) {
                        totals.lock().expect("totals").skipped += 1;
                        continue;
                    }

                    let outcome = one(&mut worker, target).and_then(|page| match page {
                        Page::Empty => std::fs::write(&marker, "").map(|_| None).map_err(|e| e.to_string()),
                        Page::Object { raw, items } => {
                            let packed = pack(&raw, Level::Stored).map_err(|e| e.to_string())?;
                            write_atomic(&path, &packed).map_err(|e| e.to_string())?;
                            Ok(Some((packed.len() as u64, items)))
                        }
                    });

                    let done = {
                        let mut t = totals.lock().expect("totals");
                        match outcome {
                            Ok(Some((bytes, items))) => {
                                t.baked += 1;
                                t.bytes += bytes;
                                t.items += items;
                            }
                            Ok(None) => t.empty += 1,
                            Err(err) => {
                                t.failed += 1;
                                eprintln!("[bake/{tag}/{}] failed: {err}", ivec::repr(page_idx));
                            }
                        }
                        t.baked + t.empty + t.skipped + t.failed
                    };

                    if done.is_multiple_of(every) {
                        let t = totals.lock().expect("totals");
                        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
                        // A skipped page cost nothing, so the rate that
                        // predicts the rest is over the pages actually done.
                        let worked = done - t.skipped;
                        let rate = if worked > 0 { elapsed / worked as f64 } else { 0.0 };
                        println!(
                            "[bake/{tag}] {done}/{} baked={} empty={} skipped={} failed={} bytes={} {rate:.0}ms/page eta={}",
                            targets.len(),
                            t.baked,
                            t.empty,
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

    let tally = totals.into_inner().expect("totals");
    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
    println!();
    println!(
        "[bake/{tag}] done in {}: baked={} empty={} skipped={} failed={}",
        fmt_dur(elapsed_ms),
        tally.baked,
        tally.empty,
        tally.skipped,
        tally.failed
    );

    Ok(Walked { tally, elapsed_ms })
}
