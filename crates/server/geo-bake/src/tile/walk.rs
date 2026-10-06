// pxl-geo-bake/tile/walk.rs

//! The reverse-geocode bake's walk over terrain's land tiles, one object per
//! group, resumable. A missing object reads as empty, so there is no shared
//! fallback.

use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::layer::tile::tile_at;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use crate::cli::Flags;
use pxl_spec::space::coord::pvf3::ivec::{self, IVec};

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

/// What one item of a walk came to.
pub enum Done {
    Wrote { bytes: u64, items: u64 },
    Empty,
    Skipped,
}

/// Every band's items, a band to a worker at a time, each worker with its own
/// `W` opened once: a band keeps in one worker what its items share (the
/// leaf stage's rows, whose neighbors' reads it keeps), an item alone
/// balances. Progress every `every` items, the rate over items not skipped.
pub fn over<T: Copy + Sync, W>(
    tag: &str,
    bands: &[Vec<T>],
    workers: usize,
    every: u64,
    show: impl Fn(T) -> String + Sync,
    open: impl Fn() -> Result<W, String> + Sync,
    one: impl Fn(&mut W, T) -> Result<Done, String> + Sync,
) -> Tally {
    let all: u64 = bands.iter().map(|b| b.len() as u64).sum();
    let next = AtomicUsize::new(0);
    let totals = Mutex::new(Tally::default());
    let started = Instant::now();
    std::thread::scope(|scope| {
        for _ in 0..workers.max(1) {
            scope.spawn(|| {
                let mut w = match open() {
                    Ok(w) => w,
                    Err(err) => return eprintln!("[bake/{tag}] worker failed to start: {err}"),
                };
                while let Some(band) = bands.get(next.fetch_add(1, Ordering::Relaxed)) {
                    for &item in band {
                        let outcome = one(&mut w, item);
                        let mut t = totals.lock().expect("totals");
                        match outcome {
                            Ok(Done::Wrote { bytes, items }) => (t.baked, t.bytes, t.items) = (t.baked + 1, t.bytes + bytes, t.items + items),
                            Ok(Done::Empty) => t.empty += 1,
                            Ok(Done::Skipped) => t.skipped += 1,
                            Err(err) => {
                                t.failed += 1;
                                eprintln!("[bake/{tag}/{}] failed: {err}", show(item));
                            }
                        }
                        let done = t.baked + t.empty + t.skipped + t.failed;
                        if done.is_multiple_of(every) {
                            let worked = done - t.skipped;
                            let rate = if worked > 0 { started.elapsed().as_secs_f64() * 1000.0 / worked as f64 } else { 0.0 };
                            println!(
                                "[bake/{tag}] {done}/{all} baked={} empty={} skipped={} failed={} {} {rate:.0}ms/tile eta={}",
                                t.baked,
                                t.empty,
                                t.skipped,
                                t.failed,
                                fmt_bytes(t.bytes as f64),
                                fmt_dur(rate * (all - done) as f64)
                            );
                        }
                    }
                }
            });
        }
    });
    let t = totals.into_inner().expect("totals");
    println!(
        "[bake/{tag}] done in {}: baked={} empty={} skipped={} failed={} {}",
        fmt_dur(started.elapsed().as_secs_f64() * 1000.0),
        t.baked,
        t.empty,
        t.skipped,
        t.failed,
        fmt_bytes(t.bytes as f64)
    );
    t
}

/// The geocode groups, one object each under its name; a group already
/// written or marked empty is skipped unless `--force`.
pub fn walk<T>(
    args: &Args,
    tag: &str,
    zoom: Zoom,
    every: u64,
    targets: &[Target],
    connect: impl Fn() -> Result<T, String> + Sync,
    one: impl Fn(&mut T, &Target) -> Result<Tile, String> + Sync,
) -> Result<Tally, String> {
    let bands: Vec<Vec<&Target>> = targets.iter().map(|t| vec![t]).collect();
    Ok(over(
        tag,
        &bands,
        args.concurrency,
        every,
        |t| ivec::repr(t.idx),
        connect,
        |worker, target| {
            let name = tile_at(zoom, [ivec::X(target.idx), ivec::Y(target.idx)]).name();
            let (path, marker) = (args.out.join(&name), args.out.join(format!("{name}{EMPTY_SUFFIX}")));
            if !args.force && (path.exists() || marker.exists()) {
                return Ok(Done::Skipped);
            }
            match one(worker, target)? {
                Tile::Empty => std::fs::write(&marker, "").map(|_| Done::Empty).map_err(|e| e.to_string()),
                Tile::Object { raw, items } => {
                    let packed = pack(&raw, Level::Stored).map_err(|e| e.to_string())?;
                    write_atomic(&path, &packed).map_err(|e| e.to_string())?;
                    Ok(Done::Wrote { bytes: packed.len() as u64, items })
                }
            }
        },
    ))
}
