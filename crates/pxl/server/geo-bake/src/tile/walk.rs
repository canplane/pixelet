// pxl-geo-bake/tile/walk.rs

//! The reverse-geocode bake's walk over terrain's land tiles, one object per
//! group, resumable. A missing object reads as empty, so there is no shared
//! fallback.

use pxl_spec::bake::quadkey::quadkey_of;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

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
            land_cache: PathBuf::from("./out/terrain/land_tiles.json"),
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

pub enum Tile {
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
    for &tile in land {
        let idx = ivec::sh(tile, -(shamt as i32));
        let i = *at.entry(idx).or_insert_with(|| {
            out.push(Target { idx, leaves: Vec::new() });
            out.len() - 1
        });
        out[i].leaves.push(tile);
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

pub fn walk<T>(
    args: &Args,
    tag: &str,
    depth: u32,
    every: u64,
    targets: &[Target],
    connect: impl Fn() -> Result<T, String> + Sync,
    one: impl Fn(&mut T, &Target) -> Result<Tile, String> + Sync,
) -> Result<Tally, String> {
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
                    let tile_idx = target.idx;
                    let name = quadkey_of(depth, tile_idx).to_string();
                    let path = args.out.join(&name);
                    let marker = args.out.join(format!("{name}{EMPTY_SUFFIX}"));

                    if !args.force && (path.exists() || marker.exists()) {
                        totals.lock().expect("totals").skipped += 1;
                        continue;
                    }

                    let outcome = one(&mut worker, target).and_then(|tile| match tile {
                        Tile::Empty => std::fs::write(&marker, "").map(|_| None).map_err(|e| e.to_string()),
                        Tile::Object { raw, items } => {
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
                                eprintln!("[bake/{tag}/{}] failed: {err}", ivec::repr(tile_idx));
                            }
                        }
                        t.baked + t.empty + t.skipped + t.failed
                    };

                    if done.is_multiple_of(every) {
                        let t = totals.lock().expect("totals");
                        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
                        // Skipped tiles cost nothing, so the rate is over tiles actually done.
                        let worked = done - t.skipped;
                        let rate = if worked > 0 { elapsed / worked as f64 } else { 0.0 };
                        println!(
                            "[bake/{tag}] {done}/{} baked={} empty={} skipped={} failed={} bytes={} {rate:.0}ms/tile eta={}",
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

    Ok(tally)
}
