// pxl-geo-bake/tile/walk.rs

//! A bake's walk over its tiles, a band of them to a worker at a time, with
//! a running count.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use crate::util::{fmt_bytes, fmt_dur};

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

/// A band to a worker at a time: a band shares its worker's `W` (a leaf row's
/// neighbors), a lone item balances.
pub fn over<T: Copy + Sync, W>(
    tag: &str,
    bands: &[&[T]],
    workers: usize,
    every: u64,
    show: impl Fn(T) -> String + Sync,
    open: impl Fn() -> Result<W, String> + Sync,
    one: impl Fn(&mut W, T) -> Result<Done, String> + Sync,
) -> Result<Tally, String> {
    let all: u64 = bands.iter().map(|b| b.len() as u64).sum();
    let next = AtomicUsize::new(0);
    let totals = Mutex::new(Tally::default());
    let refused = Mutex::new(None);
    let started = Instant::now();
    std::thread::scope(|scope| {
        for _ in 0..workers.max(1) {
            scope.spawn(|| {
                let mut w = match open() {
                    Ok(w) => w,
                    Err(err) => return *refused.lock().expect("refused") = Some(err),
                };
                while let Some(band) = bands.get(next.fetch_add(1, Ordering::Relaxed)) {
                    for &item in *band {
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
    // A worker that never started leaves its share undone, which no count shows.
    if let Some(err) = refused.into_inner().expect("refused") {
        return Err(format!("[bake/{tag}] a worker failed to start: {err}"));
    }
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
    Ok(t)
}
