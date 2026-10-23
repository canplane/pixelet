// pxl-kernel/io/fetch.rs

//! The world's reads. The kernel names an object (its bake, the version the
//! source serves it under and its name, `DemandName`) and opens it as a file; the host
//! alone knows the address and protocol. What an absent object means is decided here per stream.
//! Every body is a heap block released in the command that decodes it,
//! unless a store keeps it.

use core::fmt;

use xpute_guest::ipc::sys::Fd;
use xpute_guest::mem::base::ptr_at;
use xpute_guest::mem::heap::{alloc, free, Block};
use xpute_guest::sched::edge::{next_turn, notified, yield_if_spent, Notify};
use xpute_kit::collection::list::List;
use xpute_kit::status::errno::Errno;

use crate::io::demand::{Phases, Stream, TaskPhaseName};
use crate::io::failure::{Ended, Failure};
use crate::io::file::{self, Opened, Owner, Path};
use crate::io::request::{attempt_of, settle, Credit};
use crate::io::source;
use crate::param::MAX_CREDITS;
use crate::sched::edge::{request_world_tick, world_edge};
use pxl_spec::route::tile_object;
use pxl_spec::space::hierarchy::morton::write_name;
use pxl_spec::space::layer::tile::Tile;
use xpute_guest::clock::now;
use xpute_guest::global::Global;

pub struct DemandName(pub Stream, pub u64);

impl fmt::Display for DemandName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let key = self;
        match key.0 {
            Stream::terrain | Stream::place | Stream::earth | Stream::division => write_name::<2>(f, Tile::of_subject(key.1).key()),
            // Assets are opened below the assets' root (io/load.rs), never here.
            Stream::asset => Ok(()),
        }
    }
}

pub fn start(credit: Credit, stream: Stream, subject: u64) {
    let Some(bake) = stream.bake() else { return };
    let name = Path::of(|w| tile_object(w, source::version(bake, Tile::of_subject(subject)), DemandName(stream, subject)));
    let owner = Owner::Fetch { key: credit.key(), at_ms: now() };
    file::open(credit, bake as u32, name.as_str(), owner);
}

/// Settled as stopped at once: nothing is reported for a closed descriptor.
pub fn cancel(key: u64) -> bool {
    let Some(fd) = file::held_for(|o| matches!(o, Owner::Fetch { key: k, .. } if *k == key)) else {
        return false;
    };
    file::close(fd);
    settle(key, Ended::Aborted, None);
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

/// No more arrive at once than fetches are out, each on its credit.
static ARRIVED: Global<List<Arrival, { MAX_CREDITS as usize }>> = Global::new();

/// An answer, a body or the source's absence, kept for its owner to take:
/// what it does with it (admits a tile, opens a group) is the owner's. The
/// fetch stays out until the owner settles it.
pub struct Answered {
    pub key: u64,
    pub stream: Stream,
    pub subject: u64,
    /// None where the source has no object for it.
    pub block: Option<Block>,
    pub phases: Phases,
}

/// No more are kept than fetches are out, each on its credit, oldest first.
static ANSWERED: Global<List<Answered, { MAX_CREDITS as usize }>> = Global::new();

/// The oldest answer of one of `of`, not yet taken. No more are kept than
/// fetches are out, so the scan is short.
pub fn next_answered(of: &[Stream]) -> Option<Answered> {
    let all = ANSWERED.get(List::new);
    let at = all.iter().position(|a| of.contains(&a.stream))?;
    Some(all.remove(at))
}

/// An answer's fetch settles once its owner has taken it in, or refused it.
pub fn answer_settled(key: u64, result: Result<(), Failure>, phases: Phases) {
    settle(key, Ended::from(result), Some(phases));
}
static ARRIVALS: Global<Notify> = Global::new();

fn arrived() -> &'static mut List<Arrival, { MAX_CREDITS as usize }> {
    ARRIVED.get(List::new)
}

fn arrivals() -> &'static mut Notify {
    ARRIVALS.get(Notify::new)
}

/// `detail` is only printed by the profiler; nothing branches on it, which
/// keeps the protocol out of here.
pub fn opened(fd: Fd, key: u64, at_ms: f64, opened: Opened) {
    let got = match opened {
        Opened::Absent => Got::Absent,
        failed @ Opened::Failed { .. } => Got::Failed(failed.failure()),
        Opened::Bytes(0) => {
            file::close(fd);
            Got::Read(None)
        }
        Opened::Bytes(n) => {
            let got = match alloc(n) {
                Some(block) => {
                    // SAFETY: a block just acquired, `n` long, and no one else's;
                    // the arrival holds it until `fetch_task` waits out the turn.
                    unsafe { file::read(fd, core::slice::from_raw_parts_mut(ptr_at::<u8>(block.0), n as usize)) };
                    Got::Read(Some(block))
                }
                None => Got::Failed(Errno::ENOMEM.into()),
            };
            file::close(fd);
            got
        }
    };
    arrived().push(Arrival { key, got, net_ms: now() - at_ms });
    arrivals().notify();
}

/// Arrivals are decoded on a later turn, once their bytes are in, as many
/// as the quota allows and at least one.
pub async fn fetch_task() {
    loop {
        notified(arrivals).await;
        while !arrived().is_empty() {
            let received = core::mem::take(arrived());
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
    let Some((stream, subject)) = attempt_of(key) else {
        release(block);
        return;
    };
    let mut phases = Phases::default();
    phases.push(TaskPhaseName::net_ms, net_ms);
    if let Some((_, bytes)) = block {
        phases.push(TaskPhaseName::bytes, bytes as f64);
    }
    let fail = match got {
        // Taken by its owner, which settles it; and a turn asked, so one takes it.
        Got::Absent | Got::Read(Some(_)) => {
            // An asset loads through io/load, never a fetch: nothing would take its answer.
            xpute_kit::ensure!(stream != Stream::asset, ENOTRECOVERABLE);
            ANSWERED.get(List::new).push(Answered { key, stream, subject, block, phases });
            request_world_tick();
            return;
        }
        // Only an absence says there is none; an empty read is not an object.
        Got::Read(None) => Failure::from(Errno::EBADMSG),
        Got::Failed(fail) => fail,
    };
    settle(key, Ended::Failed(fail), Some(phases));
}
