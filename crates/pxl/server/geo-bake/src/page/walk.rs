// pxl-geo-bake/page/walk.rs

//! The walk the POI and reverse-geocode bakes share: terrain's own land-page
//! list, one object per page, resumable.
//!
//! Reusing terrain's land/ocean split rather than a classification of their own
//! is deliberate. "Does this bbox touch any land polygon" is a cheap, purely
//! geometric pre-filter that already rules out every page these could never
//! produce anything for; a real "does this page have a POI-taggable feature"
//! pass would cost about what fetching costs, so there is nothing cheaper to
//! filter on than what terrain already computed. A land page with nothing on it
//! still costs one round trip — the `.empty` marker's job is to make that a
//! one-time cost, not to remove it.
//!
//! Neither bake has a shared fallback the way terrain's `_ocean` does: "nothing
//! here" is an empty result rather than a payload some other page's bytes could
//! stand in for, so a missing key is enough on its own and the client reads a
//! 404 as empty directly.
//!
//! The terrain bake does not go through this. Its walk is over a candidate grid
//! it classifies itself, it has the shared ocean tile, and it has no `.empty` —
//! folding it in would bend all three.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use pxl_spec::space::pvf3::ivec::{self, IVec};

use crate::util::{fmt_bytes, fmt_dur, gzip, write_atomic};

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

/// What one page came to. `items` is whatever the caller counts per object —
/// places for POI, nothing for a label.
pub enum Page {
    Object { raw: Vec<u8>, items: u64 },
    Empty,
}

#[derive(Default)]
pub struct Tally {
    pub baked: u64,
    pub empty: u64,
    pub skipped: u64,
    pub failed: u64,
    pub items: u64,
    pub gz: u64,
}

pub struct Walked {
    pub tally: Tally,
    /// Every land page the cache named, not only the ones this run touched.
    pub land: Vec<IVec>,
    pub elapsed_ms: f64,
}

pub fn land_pages(path: &Path) -> Result<Vec<IVec>, String> {
    let raw = std::fs::read_to_string(path).map_err(|_| {
        format!(
            "{} not found — run bake_tiles first, which writes it as a side effect of its own classification (`--limit 0` does the classification and bakes nothing).",
            path.display()
        )
    })?;
    let list: Vec<String> = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    list.iter().map(|s| s.parse::<IVec>().map_err(|e| e.to_string())).collect()
}

/// Walks the pages with one worker per connection, writing what `one` returns.
///
/// `connect` is called once per worker, so a worker owns its connection and its
/// prepared statements for the whole run.
pub fn walk<T>(args: &Args, tag: &str, every: u64, connect: impl Fn() -> Result<T, String> + Sync, one: impl Fn(&mut T, IVec) -> Result<Page, String> + Sync) -> Result<Walked, String> {
    std::fs::create_dir_all(&args.out).map_err(|e| e.to_string())?;
    let land = land_pages(&args.land_cache)?;
    println!("[bake/{tag}] {} land pages from {}", land.len(), args.land_cache.display());

    let targets: &[IVec] = match args.limit {
        Some(n) => &land[..n.min(land.len())],
        None => &land,
    };

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
                    let Some(&page_idx) = targets.get(i) else { break };
                    let path = args.out.join(page_idx.to_string());
                    let marker = args.out.join(format!("{page_idx}{EMPTY_SUFFIX}"));

                    if !args.force && (path.exists() || marker.exists()) {
                        totals.lock().expect("totals").skipped += 1;
                        continue;
                    }

                    let outcome = one(&mut worker, page_idx).and_then(|page| match page {
                        Page::Empty => std::fs::write(&marker, "").map(|_| None).map_err(|e| e.to_string()),
                        Page::Object { raw, items } => {
                            let gz = gzip(&raw).map_err(|e| e.to_string())?;
                            write_atomic(&path, &gz).map_err(|e| e.to_string())?;
                            Ok(Some((gz.len() as u64, items)))
                        }
                    });

                    let done = {
                        let mut t = totals.lock().expect("totals");
                        match outcome {
                            Ok(Some((gz, items))) => {
                                t.baked += 1;
                                t.gz += gz;
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
                            "[bake/{tag}] {done}/{} baked={} empty={} skipped={} failed={} gz={} {rate:.0}ms/page eta={}",
                            targets.len(),
                            t.baked,
                            t.empty,
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

    Ok(Walked { tally, land, elapsed_ms })
}
