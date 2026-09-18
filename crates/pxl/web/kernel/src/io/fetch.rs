// pxl-kernel/io/fetch.rs

//! The world's fetches at the shell's edge. The shell runs HTTP and nothing
//! else: for a demand the scheduler started it is told the URL
//! (`demand_url`); when a response arrives it keeps the body and reports the
//! status and the body's length (`fetch_queued`), and `fetch_task`, on a
//! turn's quota, takes a block and has the host copy the body into it — the
//! way a driver reads what a device holds — and decodes it. Which category a
//! key is, what a 404 means and how a body decodes are decided here.
//!
//! Every body is a heap block the kernel owns throughout. A body is decoded or
//! read and released in the same command. A terrain body does not outlive its
//! command — the page it carries is written into `.fixed.terrain`, whose
//! record is the page's own coordinate — and a POI page still keeps its body
//! as the page, until its per-page index is rewritten (issue/BACKLOG.md).

use core::fmt;

use pxl_spec::fetch::FetchFail;
use xpute_core::kit::array::{ArrayBuffer, U8Array};
use xpute_runtime::sched::edge::{notified, yield_if_spent, Notify};
use xpute_runtime::sys::mem::{alloc, free, Block};

use crate::io::demand::{Category, Demand, Phases};
use crate::io::fetch_scheduler::{demand_of, fetch_settle};
use crate::layer::poi::{repository as poi_repository, store as poi_store};
use crate::layer::terrain::{tile_fetcher, tile_repository};
use crate::sched::ledger::record::{Failure, Outcome};
use crate::shell::config::env::PUBLISHED_TILES;
use crate::sys::global::Global;
use crate::sys::host::body_read;
use crate::world::change::world_edge;

/// Where a started demand is fetched from, as it is written where it is sent.
pub struct DemandUrl<'a>(pub &'a Demand);

impl fmt::Display for DemandUrl<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let key = self.0.key;
        match key.category {
            Category::Terrain => write!(f, "{}", tile_repository::tile_url(key.subject)),
            // The one tile the stream has, at the name the bake gives it.
            Category::Ocean => write!(f, "{}", tile_repository::ocean_url()),
            Category::Poi => write!(f, "{}", poi_repository::poi_url(key.subject)),
        }
    }
}

/// The body the host holds as `body`, `bytes` long, copied into a block of
/// its own: None when there is none, NoRoom when the heap has no block for it.
/// The host lets the body go either way.
fn receive((body, bytes): (u32, u32)) -> Result<Option<Block>, FetchFail> {
    if bytes == 0 {
        return Ok(None);
    }
    let Some(block) = alloc(bytes) else {
        body_read(body, &mut []);
        return Err(FetchFail::NoRoom);
    };
    // SAFETY: a block just acquired, `bytes` long, and no one else's.
    body_read(body, unsafe { core::slice::from_raw_parts_mut(block.0 as *mut u8, bytes as usize) });
    Ok(Some(block))
}

fn release(block: Option<Block>) {
    if let Some(block) = block {
        // SAFETY: a body's block, acquired by `receive` and released once.
        unsafe { free(block) };
    }
}

/// A body's bytes, read where the host copied them.
fn view((at, bytes): Block) -> U8Array {
    // SAFETY: the block is one `receive` acquired for this body alone,
    // `bytes` long, and it is not released while the view is read.
    let buffer = unsafe { ArrayBuffer::from_raw(at as *mut u8, bytes as usize) };
    U8Array::over(buffer, 0, bytes as usize)
}

/// The shell's report that a started demand's fetch finished: `outcome` 0
/// A finished fetch as the shell reported it, waiting for `fetch_task`.
#[derive(Clone, Copy)]
struct Arrival {
    key: u64,
    outcome: u32,
    status: u32,
    body: (u32, u32),
    fail: u32,
    net_ms: f64,
}

static ARRIVED: Global<Vec<Arrival>> = Global::new();
static ARRIVALS: Global<Notify> = Global::new();

fn arrived() -> &'static mut Vec<Arrival> {
    ARRIVED.get(Vec::new)
}

fn arrivals() -> &'static mut Notify {
    ARRIVALS.get(Notify::new)
}

/// The shell's report that a started demand's fetch finished, queued for
/// `fetch_task`: `outcome` 0 with the response's status and the body it
/// holds, `(body, bytes)`, 1 aborted, 2 failed with `fail`.
pub fn fetch_queued(key: u64, outcome: u32, status: u32, body: (u32, u32), fail: u32, net_ms: f64) {
    arrived().push(Arrival {
        key,
        outcome,
        status,
        body,
        fail,
        net_ms,
    });
    arrivals().notify();
}

/// The fetches' task: each arrival read, decoded and settled, one a turn at
/// least and as many as the quota lets. A body is one unit.
pub async fn fetch_task() {
    loop {
        notified(arrivals).await;
        while !arrived().is_empty() {
            yield_if_spent(world_edge).await;
            let a = arrived().remove(0);
            fetch_arrived(a.key, a.outcome, a.status, a.body, a.fail, a.net_ms);
        }
    }
}

/// with the response's status and the body it holds, `(body, bytes)`, 1
/// aborted, 2 failed with `fail`.
fn fetch_arrived(key: u64, outcome: u32, status: u32, body: (u32, u32), fail: u32, net_ms: f64) {
    let (outcome, fail, block) = match receive(body) {
        Ok(block) => (outcome, fail, block),
        Err(no_room) => (2, no_room as u32, None),
    };
    // A hash the scheduler no longer holds is one a reset forgot; its body is
    // already back.
    let Some(name) = demand_of(key) else {
        release(block);
        return;
    };
    let (category, subject) = (name.category, name.subject);
    let mut phases = Phases::default();
    phases.push("net_ms", net_ms);
    if let Some((_, bytes)) = block {
        phases.push("bytes", bytes as f64);
    }
    let (outcome, fail) = match outcome {
        0 => match answer(category, subject, status, block, &mut phases) {
            Ok(()) => (Outcome::Ok, FetchFail::None),
            Err(fail) => (Outcome::Error, fail),
        },
        1 => {
            release(block);
            (Outcome::Aborted, FetchFail::None)
        }
        // The shell's own failure, as the number it sent. A code this build
        // does not know reads as one the shell raised rather than as none.
        _ => {
            release(block);
            (Outcome::Error, fetch_fail_of(fail))
        }
    };
    if outcome != Outcome::Ok {
        failed(category, outcome == Outcome::Aborted);
    }
    fetch_settle(name, outcome, Some(phases), Failure { fail, status: status as u16 });
}

fn failed(category: Category, aborted: bool) {
    match category {
        Category::Terrain if !aborted => tile_fetcher::terrain_failed(),
        _ => {}
    }
}

/// What a response means for its category. The body is released here, unless
/// a POI page keeps it.
/// The reason a number names, or `Shell` for one this build does not know —
/// an older kernel reading a newer shell still counts the failure apart from
/// the rest.
fn fetch_fail_of(fail: u32) -> FetchFail {
    const KNOWN: [FetchFail; 5] = [FetchFail::Network, FetchFail::Shell, FetchFail::Status, FetchFail::NoRoom, FetchFail::Decode];
    KNOWN.into_iter().find(|f| *f as u32 == fail).unwrap_or(FetchFail::Shell)
}

fn answer(category: Category, subject: u64, status: u32, block: Option<Block>, phases: &mut Phases) -> Result<(), FetchFail> {
    // Only a static tile origin has no object for a page; against the live
    // API a 404 is a genuine error (tile_repository.rs).
    let absent = status == 404 && PUBLISHED_TILES();
    let ok = (200..300).contains(&status);
    let refused = || FetchFail::Status;
    match category {
        Category::Terrain => {
            let result = match block {
                _ if absent => {
                    tile_repository::ocean_page(subject);
                    Ok(())
                }
                Some(b) if ok => tile_repository::adopt_terrain(subject, view(b), phases),
                _ => Err(refused()),
            };
            release(block);
            result
        }
        Category::Ocean => {
            let result = match block {
                Some(b) if ok => tile_repository::adopt_ocean(view(b)),
                _ => Err(refused()),
            };
            release(block);
            result
        }
        Category::Poi => {
            let page = subject;
            match block {
                _ if absent => {
                    release(block);
                    poi_store::set_pois(page, poi_repository::empty_page().map_err(|_| FetchFail::Decode)?);
                    Ok(())
                }
                // SAFETY: the block is the heap's, given for this body
                // alone; the page keeps it and the store releases it with the page.
                Some((at, bytes)) if ok => match unsafe { poi_repository::open_poi_page(at, bytes) } {
                    Ok(resident) => {
                        poi_store::set_pois(page, resident);
                        Ok(())
                    }
                    Err(_) => {
                        release(block);
                        Err(FetchFail::Decode)
                    }
                },
                _ => {
                    release(block);
                    Err(refused())
                }
            }
        }
    }
}
