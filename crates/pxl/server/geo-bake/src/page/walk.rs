// pxl-geo-bake/page/walk.rs

//! The reverse-geocode bake's walk over terrain's land pages, one object per
//! group, resumable. A missing object reads as empty, so there is no shared
//! fallback.

use pxl_spec::bake::quadkey::quadkey_of;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use pxl_spec::space::indexer::L3;

use crate::cli::Flags;
use pxl_spec::space::pvf3::ivec::{self, IVec};

use crate::util::{fmt_bytes, fmt_dur, pack, write_atomic, Level};

/// "Already asked, nothing here", for resume; never uploaded.
pub const EMPTY_SUFFIX: &str = ".empty";

pub struct Args {
    pub out: PathBuf,
    pub land_cache: PathBuf,
    pub concurrency: usize,
    pub limit: Option<usize>,
    pub force: bool,
}

impl Args {
    pub fn parse(default_out: PathBuf) -> Result<Args, String> {
        let mut args = Args {
            out: default_out,
            land_cache: PathBuf::from("./out/terrain/land_pages.json"),
            concurrency: 8,
            limit: None,
            force: false,
        };

        let mut flags = Flags::from_env();
        while let Some(flag) = flags.arg() {
            match flag.as_str() {
                "--out" => args.out = PathBuf::from(flags.value()?),
                "--land-cache" => args.land_cache = PathBuf::from(flags.value()?),
                "--concurrency" => args.concurrency = flags.parse()?,
                "--limit" => args.limit = Some(flags.parse()?),
                "--force" => args.force = true,
                other => return Err(format!("unknown flag: {other}")),
            }
        }
        Ok(args)
    }
}

pub enum Page {
    Object { raw: Vec<u8>, items: u64 },
    Empty,
}

pub struct Target {
    pub idx: IVec,
    pub leaves: Vec<IVec>,
}

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

/// Refused at any page size but the one it was classified at: a stale list
/// names the wrong ground and still bakes.
pub fn land_pages(path: &Path) -> Result<Vec<IVec>, String> {
    let reclassify = "run bake_tiles, which writes it as a side effect of its own classification (`--limit 0` does the classification and bakes nothing)";
    let raw = std::fs::read_to_string(path).map_err(|_| format!("{} not found — {reclassify}.", path.display()))?;
    let v: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    let Some(shamt) = v.get("l3_shamt").and_then(|s| s.as_u64()) else {
        return Err(format!("{} does not say the page size it was classified at — {reclassify}.", path.display()));
    };
    if shamt != L3.shamt() as u64 {
        return Err(format!("{} was classified at an L3 shift of {shamt}, and a page is now {} — {reclassify}.", path.display(), L3.shamt()));
    }
    let list = v.get("pages").and_then(|p| p.as_array()).ok_or_else(|| format!("{} has no page list", path.display()))?;
    list.iter().map(|s| s.as_str().ok_or("a page is not a string")?.parse::<IVec>().map_err(|e| e.to_string())).collect()
}

pub fn write_land_pages(path: &Path, land: &[IVec]) -> Result<(), String> {
    let pages: Vec<String> = land.iter().map(|p| p.to_string()).collect();
    let v = serde_json::json!({ "l3_shamt": L3.shamt(), "pages": pages });
    std::fs::write(path, serde_json::to_string(&v).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

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
                        // Skipped pages cost nothing, so the rate is over pages actually done.
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
