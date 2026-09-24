// pxl-kernel/io/fetch.rs

//! The world's reads at the shell's edge. What the kernel asks for is an
//! **object it names** — which bake it belongs to and its name inside that
//! bake (`DemandName`) — and the shell is the one that knows what address
//! that is and what protocol reaches it. Nothing here spells a URL, a scheme
//! or a status.
//!
//! What comes back is one of four: the bytes, no such object, the read was
//! stopped, the read failed. The host decides which — a protocol's own way of
//! saying "there is none" is the host's to read — and `fetch_task`, on a
//! turn's quota, takes a block and has the host copy the body into it, the
//! way a driver reads what a device holds, and decodes it. What an absent
//! object *means* for its category is decided here, and that is world
//! knowledge rather than transport.
//!
//! Every body is a heap block the kernel owns throughout. A body is decoded or
//! read and released in the same command. A terrain body does not outlive its
//! command — the page it carries is written into `.fixed.terrain`, whose
//! record is the page's own coordinate. POI summary packets are decoded into
//! bounded records and their bodies are released in the same command.

use core::fmt;

use xpute_runtime::abi::fetch::FetchFail;
use xpute_runtime::mem::heap::{alloc, free, Block};
use xpute_runtime::sched::edge::{notified, yield_if_spent, Notify};

use crate::global::Global;
use crate::host::body_read;
use crate::io::demand::{Category, Phases};
use crate::io::policy;
use crate::io::sched::{attempt_of, settle};
use crate::layer::geocode::{repository as geocode_repository, store as geocode_store};
use crate::layer::poi::store as poi_store;
use crate::layer::terrain::tile_repository;
use crate::sched::ledger::record::{Failure, Outcome};
use crate::world::change::world_edge;
use pxl_spec::space::pyramid::LevelPage;

/// A started attempt's object, named inside its bake — what is written where
/// the read is asked for. Which bake is `Category::bake`, and the two
/// together are all the host is given.
pub struct DemandName(pub Category, pub u64);

impl fmt::Display for DemandName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let key = self;
        match key.0 {
            Category::Terrain => write!(f, "{}", tile_repository::TileName(LevelPage::of_subject(key.1))),
            // The one tile the stream has, at the name the bake gives it.
            Category::Ocean => write!(f, "{}", tile_repository::OCEAN_KEY),
            Category::PoiDetail => f.write_str(&crate::layer::poi::detail::name(key.1)),
            Category::Poi => f.write_str(&pxl_spec::poi::xy::name(key.1)),
            Category::Geocode => write!(f, "{}", geocode_repository::group_name(key.1)),
            // The one packet the stream has, at the name the bake gives it.
            Category::GeocodeDict => write!(f, "{}", geocode_repository::dict_name()),
            // An asset is named by its content and belongs to no bake, so it
            // is asked for by its own command and never written here.
            Category::Asset => Ok(()),
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
fn view<'a>((at, bytes): Block) -> &'a [u8] {
    // SAFETY: the block is one `receive` acquired for this body alone,
    // `bytes` long, and it is not released while the view is read.
    unsafe { core::slice::from_raw_parts(at as *const u8, bytes as usize) }
}

/// A finished read as the host reported it, waiting for `fetch_task`.
#[derive(Clone, Copy)]
struct Arrival {
    key: u64,
    outcome: u32,
    body: (u32, u32),
    fail: u32,
    /// Whatever number the host refused with, kept for the ledger to print.
    /// Nothing branches on it, which is what keeps a protocol out of here.
    detail: u32,
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

/// The host's report that a read finished, queued for `fetch_task`:
/// `outcome` 0 with the body it holds, `(body, bytes)`, 1 stopped, 2 failed
/// with `fail`, 3 no such object.
pub fn fetch_queued(key: u64, outcome: u32, body: (u32, u32), fail: u32, detail: u32, net_ms: f64) {
    arrived().push(Arrival {
        key,
        outcome,
        body,
        fail,
        detail,
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
            fetch_arrived(a.key, a.outcome, a.body, a.fail, a.detail, a.net_ms);
        }
    }
}

fn fetch_arrived(key: u64, outcome: u32, body: (u32, u32), fail: u32, detail: u32, net_ms: f64) {
    if attempt_of(key).is_some_and(|(c, _)| match c {
        Category::Poi => body.1 as usize > pxl_spec::poi::wire::MAX_PACKET_BYTES,
        Category::PoiDetail => body.1 as usize > pxl_spec::poi::detail::MAX_PACKET_BYTES,
        _ => false,
    }) {
        body_read(body.0, &mut []);
        settle(key, Outcome::Error, None, Failure { fail: FetchFail::Decode, detail: 0 }, policy::retry_after(Outcome::Error));
        return;
    }
    let (outcome, fail, block) = match receive(body) {
        Ok(block) => (outcome, fail, block),
        Err(no_room) => (2, no_room as u32, None),
    };
    // A hash the scheduler does not hold is one a reset forgot; its body is
    // already back.
    let Some((category, subject)) = attempt_of(key) else {
        release(block);
        return;
    };
    let mut phases = Phases::default();
    phases.push("net_ms", net_ms);
    if let Some((_, bytes)) = block {
        phases.push("bytes", bytes as f64);
    }
    let (outcome, fail) = match outcome {
        0 => match adopt(category, subject, block, &mut phases) {
            Ok(()) => (Outcome::Ok, FetchFail::None),
            Err(fail) => (Outcome::Error, fail),
        },
        // There is no such object, and for most categories that is an answer
        // rather than a fault — which of the two is the category's own and
        // is the one piece of this the world decides.
        3 => {
            release(block);
            match absent(category, subject) {
                Ok(()) => (Outcome::Ok, FetchFail::None),
                Err(fail) => (Outcome::Error, fail),
            }
        }
        1 => {
            release(block);
            (Outcome::Aborted, FetchFail::None)
        }
        // The host's own failure, as the number it sent. A code this build
        // does not know reads as one the host raised rather than as none.
        _ => {
            release(block);
            (Outcome::Error, fetch_fail_of(fail))
        }
    };
    settle(key, outcome, Some(phases), Failure { fail, detail: detail as u16 }, policy::retry_after(outcome));
}

/// The reason a number names, or `Shell` for one this build does not know —
/// an older kernel reading a newer shell still counts the failure apart from
/// the rest.
fn fetch_fail_of(fail: u32) -> FetchFail {
    const KNOWN: [FetchFail; 5] = [FetchFail::Network, FetchFail::Shell, FetchFail::Refused, FetchFail::NoRoom, FetchFail::Decode];
    KNOWN.into_iter().find(|f| *f as u32 == fail).unwrap_or(FetchFail::Shell)
}

/// There is no such object. What that means is the category's: open water
/// for a terrain page, no label for a geocode group. POI snapshots must exist.
/// The shared ocean tile and the dictionary are the two that must exist, so
/// their absence is a fault.
fn absent(category: Category, subject: u64) -> Result<(), FetchFail> {
    match category {
        Category::Terrain => tile_repository::ocean_page(LevelPage::of_subject(subject)),
        Category::Poi | Category::PoiDetail => return Err(FetchFail::Refused),
        Category::Geocode => geocode_store::set_group(subject, None),
        // A bake with no dictionary is one whose groups name nothing: an
        // answer, so the demand is not made again.
        Category::GeocodeDict => geocode_store::set_dict(None),
        Category::Ocean => return Err(FetchFail::Refused),
        // An asset's read is settled where it is asked for (io/load.rs) and
        // never arrives here.
        Category::Asset => return Err(FetchFail::Shell),
    };
    Ok(())
}

/// The bytes arrived. The block is released here, unless the page keeps it.
fn adopt(category: Category, subject: u64, block: Option<Block>, phases: &mut Phases) -> Result<(), FetchFail> {
    // A read that answered with nothing is not an object; only `absent` says
    // there is none, and it carries no body.
    let Some(b) = block else { return Err(FetchFail::Decode) };
    match category {
        Category::Terrain => {
            let result = tile_repository::adopt_terrain(LevelPage::of_subject(subject), view(b), phases);
            release(block);
            result
        }
        Category::Ocean => {
            let result = tile_repository::adopt_ocean(view(b));
            release(block);
            result
        }
        // SAFETY of each `open_*` below: the block is the heap's, given for
        // this body alone; the page or packet keeps it and the store releases
        // it with them.
        Category::PoiDetail => {
            let result = crate::layer::poi::detail::adopt(subject, view(b)).map_err(|_| FetchFail::Decode);
            release(block);
            result
        }
        Category::Poi => {
            let result = poi_store::adopt(subject, view(b)).map_err(|_| FetchFail::Decode);
            release(block);
            result
        }
        Category::Geocode => match unsafe { geocode_repository::open_group(b.0, b.1) } {
            Ok(resident) => {
                geocode_store::set_group(subject, Some(resident));
                Ok(())
            }
            Err(_) => {
                release(block);
                Err(FetchFail::Decode)
            }
        },
        // As `absent`: an asset never arrives here.
        Category::Asset => Err(FetchFail::Shell),
        Category::GeocodeDict => match unsafe { geocode_repository::open_dict(b.0, b.1) } {
            Ok(resident) => {
                geocode_store::set_dict(Some(resident));
                Ok(())
            }
            Err(_) => {
                release(block);
                Err(FetchFail::Decode)
            }
        },
    }
}
