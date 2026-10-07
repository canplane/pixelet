// pxl-kernel/io/fetch.rs

//! The world's reads. The kernel names an object (its bake and its name in
//! it, `DemandName`) and opens it as a file; the host alone knows the address
//! and protocol. What an absent object means is decided here per category.
//! Every body is a heap block released in the command that decodes it,
//! unless a store keeps it.

use core::fmt;
use std::collections::VecDeque;

use xpute_core::status::errno::Errno;
use xpute_guest::ipc::sys::Fd;
use xpute_guest::mem::base::ptr_at;
use xpute_guest::mem::heap::{alloc, free, Block};
use xpute_guest::sched::edge::{next_turn, notified, yield_if_spent, Notify};

use crate::io::demand::{Category, Phases, TaskPhaseName};
use crate::io::failure::{Failure, Outcome};
use crate::io::file::{self, Owner};
use crate::io::sched::{attempt_of, settle, Credit};
use crate::layer::geocode::store as geocode_store;
use crate::sched::edge::{request_world_tick, world_edge};
use pxl_spec::bake::place_tile::{group_key, DICT_OBJECT};
use pxl_spec::space::hierarchy::morton::write_name;
use pxl_spec::space::layer::tile::Tile;
use xpute_guest::clock::now;
use xpute_guest::global::Global;

pub struct DemandName(pub Category, pub u64);

impl fmt::Display for DemandName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let key = self;
        match key.0 {
            Category::terrain | Category::poi | Category::earth => write_name::<2>(f, Tile::of_subject(key.1).key()),
            Category::geocode => write_name::<2>(f, group_key(key.1)),
            Category::geocode_dict => f.write_str(DICT_OBJECT),
            // Assets are opened below the assets' root (io/load.rs), never here.
            Category::asset => Ok(()),
        }
    }
}

pub fn start(credit: Credit, category: Category, subject: u64) {
    let Some(bake) = category.bake() else { return };
    let name = DemandName(category, subject).to_string();
    let owner = Owner::Fetch { key: credit.key(), at_ms: now() };
    file::open(credit, bake as u32, &name, owner);
}

/// Settled as stopped at once: nothing is reported for a closed descriptor.
pub fn cancel(key: u64) -> bool {
    let Some(fd) = file::held_for(|o| matches!(o, Owner::Fetch { key: k, .. } if *k == key)) else {
        return false;
    };
    file::close(fd);
    settle(key, Outcome::aborted, None, Failure::default());
    true
}

pub fn release(block: Option<Block>) {
    if let Some(block) = block {
        // SAFETY: a body's block, acquired by `opened` and released once.
        unsafe { free(block) };
    }
}

pub fn body<'a>((at, bytes): Block) -> &'a [u8] {
    // SAFETY: the block `opened` acquired for this body alone, `bytes` long,
    // not released while the view is read.
    unsafe { core::slice::from_raw_parts(ptr_at::<u8>(at), bytes as usize) }
}

enum Got {
    /// None for an empty file.
    Read(Option<Block>),
    Absent,
    Failed(Failure),
}

struct Arrival {
    key: u64,
    got: Got,
    net_ms: f64,
}

static ARRIVED: Global<Vec<Arrival>> = Global::new();

/// A pyramid tile's answer, kept for the view to admit: which tile it evicts
/// is the view's (`world/pyramid.rs` `admit_arrived`).
pub struct TileArrival {
    pub key: u64,
    pub category: Category,
    pub tile: Tile,
    /// None where the source has no object for it.
    pub block: Option<Block>,
    pub phases: Phases,
}

static TILES: Global<VecDeque<TileArrival>> = Global::new();

/// The oldest tile arrived and not yet admitted.
pub fn next_tile() -> Option<TileArrival> {
    TILES.get(VecDeque::new).pop_front()
}

/// A tile's fetch settles once it is admitted, or refused.
pub fn tile_settled(key: u64, result: Result<(), Failure>, phases: Phases) {
    let (outcome, fail) = match result {
        Ok(()) => (Outcome::ok, Failure::default()),
        Err(fail) => (Outcome::error, fail),
    };
    settle(key, outcome, Some(phases), fail);
}
static ARRIVALS: Global<Notify> = Global::new();

fn arrived() -> &'static mut Vec<Arrival> {
    ARRIVED.get(Vec::new)
}

fn arrivals() -> &'static mut Notify {
    ARRIVALS.get(Notify::new)
}

/// `detail` is only printed by the profiler; nothing branches on it, which
/// keeps the protocol out of here.
pub fn opened(fd: Fd, key: u64, at_ms: f64, res: i32, detail: u32) {
    let got = if res < 0 {
        file::close(fd);
        failure(res, detail)
    } else if res == 0 {
        file::close(fd);
        Got::Read(None)
    } else if let Some(block) = alloc(res as u32) {
        // SAFETY: a block just acquired, `res` long, and no one else's.
        file::read(fd, unsafe { core::slice::from_raw_parts_mut(ptr_at::<u8>(block.0), res as usize) });
        file::close(fd);
        Got::Read(Some(block))
    } else {
        file::close(fd);
        Got::Failed(Errno::ENOMEM.into())
    };
    arrived().push(Arrival { key, got, net_ms: now() - at_ms });
    arrivals().notify();
}

fn failure(res: i32, detail: u32) -> Got {
    if res == Errno::ENOENT as i32 {
        return Got::Absent;
    }
    Got::Failed(Failure {
        errno: res,
        detail: detail as u16,
        at: None,
    })
}

/// Arrivals are decoded on a later turn, once their bytes are in, as many
/// as the quota allows and at least one.
pub async fn fetch_task() {
    loop {
        notified(arrivals).await;
        while !arrived().is_empty() {
            let received: Vec<_> = std::mem::take(arrived());
            next_turn(world_edge).await;
            for arrival in received {
                yield_if_spent(world_edge).await;
                fetch_arrived(arrival);
            }
        }
    }
}

fn fetch_arrived(Arrival { key, got, net_ms }: Arrival) {
    let block = match got {
        Got::Read(block) => block,
        _ => None,
    };
    // A key the request scheduler does not hold is one a reset forgot.
    let Some((category, subject)) = attempt_of(key) else {
        release(block);
        return;
    };
    let mut phases = Phases::default();
    phases.push(TaskPhaseName::net_ms, net_ms);
    if let Some((_, bytes)) = block {
        phases.push(TaskPhaseName::bytes, bytes as f64);
    }
    let pyramid = matches!(category, Category::terrain | Category::poi | Category::earth);
    let tile = matches!(got, Got::Absent) || block.is_some();
    if pyramid && tile {
        let tile = Tile::of_subject(subject);
        TILES.get(VecDeque::new).push_back(TileArrival { key, category, tile, block, phases });
        request_world_tick();
        return;
    }
    let (outcome, fail) = match got {
        Got::Read(_) => match adopt(category, subject, block, &mut phases) {
            Ok(()) => (Outcome::ok, Failure::default()),
            Err(fail) => (Outcome::error, fail),
        },
        Got::Absent => match absent(category, subject) {
            Ok(()) => (Outcome::ok, Failure::default()),
            Err(fail) => (Outcome::error, fail.into()),
        },
        Got::Failed(fail) => (Outcome::error, fail),
    };
    settle(key, outcome, Some(phases), fail);
}

/// The tile over it for a pyramid tile, no label for a geocode group.
fn absent(category: Category, subject: u64) -> Result<(), Errno> {
    match category {
        Category::geocode => geocode_store::absent_group(subject),
        // A bake with no dictionary names nothing: an answer, so not asked again.
        Category::geocode_dict => geocode_store::absent_dict(),
        // A pyramid tile's absence is admitted by the view.
        Category::terrain | Category::poi | Category::earth | Category::asset => return Err(Errno::ENOTRECOVERABLE),
    };
    Ok(())
}

/// The block is released here unless the tile keeps it.
fn adopt(category: Category, subject: u64, block: Option<Block>, _phases: &mut Phases) -> Result<(), Failure> {
    // Only `absent` says there is none; an empty read is not an object.
    let Some(b) = block else { return Err(Errno::EBADMSG.into()) };
    match category {
        // SAFETY (both): the heap block given for this body alone; the store
        // keeps it, or frees it if it does not open.
        Category::geocode => unsafe { geocode_store::adopt_group(subject, b) }.map_err(Into::into),
        Category::geocode_dict => unsafe { geocode_store::adopt_dict(b) }.map_err(Into::into),
        // A pyramid tile is admitted by the view.
        Category::terrain | Category::poi | Category::earth | Category::asset => Err(Errno::ENOTRECOVERABLE.into()),
    }
}
