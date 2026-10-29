// pxl-kernel/sys/io/fetch.rs

//! The world's reads. The kernel opens a tile as its bake's object by its
//! subject (sys/io/file.rs); its version, address and protocol are the
//! client's (ts/client/src/host/io/tile.ts). What an absent object means is decided here per stream.
//! Every body is a block (`sys/io/body.rs`) released in the command that
//! decodes it, unless a store keeps it.

use xpute_guest::ipc::sys::Fd;
use xpute_guest::mem;
use xpute_guest::mem::heap::Block;
use xpute_guest::sched::edge::{next_turn, notified, yield_if_spent, Notify};
use xpute_kit::collection::list::List;
use xpute_kit::status::errno::Errno;

use crate::sys::io::body;
use crate::sys::io::demand::{Phases, Stream, TaskPhaseName};
use crate::sys::io::failure::{Ended, Failure};
use crate::sys::io::file::{self, Opened, Owner};
use crate::sys::io::param::MAX_CREDITS;
use crate::sys::io::request::{attempt_of, settle, Credit};
use crate::sys::io::room::Refused;
use crate::sys::sched::edge::{request_world_tick, world_edge};
use xpute_guest::clock::now;
use xpute_guest::global::Global;

/// Assets are opened below the assets' root (sys/io/loader.rs), never here.
pub fn start(credit: Credit, stream: Stream, subject: u64) {
    let Some(bake) = stream.bake() else { return };
    let owner = Owner::Fetch {
        key: credit.key(),
        at_ms: now(),
    };
    file::open(credit, bake as u32, subject, owner);
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
        // SAFETY: a body's block, acquired by `on_opened` and released once.
        unsafe { body::give(block) };
    }
}

pub fn body<'a>((at, bytes): Block) -> &'a [u8] {
    // SAFETY: the block `on_opened` acquired for this body alone, `bytes` long,
    // not released while the view is read.
    unsafe { core::slice::from_raw_parts(mem::at::<u8>(at), bytes as usize) }
}

enum Got {
    /// None for an empty file.
    Read(Option<Block>),
    Absent,
    Failed(Failure),
    /// The body was refused room (sys/io/body.rs `take`).
    Refused(Refused),
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

/// How many answers of one of `of` are not yet taken.
pub fn answered(of: &[Stream]) -> usize {
    ANSWERED
        .get(List::new)
        .iter()
        .filter(|a| of.contains(&a.stream))
        .count()
}

/// The oldest answer of one of `of`, not yet taken. No more are kept than
/// fetches are out, so the scan is short.
pub fn take_answered(of: &[Stream]) -> Option<Answered> {
    let all = ANSWERED.get(List::new);
    let at = all.iter().position(|a| of.contains(&a.stream))?;
    Some(all.remove(at))
}

/// An answer's fetch settles once its owner has taken it in, or refused it.
pub fn on_answer_settled(key: u64, result: Result<(), Failure>, phases: Phases) {
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
pub fn on_opened(fd: Fd, key: u64, at_ms: f64, opened: Opened) {
    let stream = attempt_of(key).map(|(stream, _)| stream);
    let got = match opened {
        Opened::Absent => Got::Absent,
        failed @ Opened::Failed { .. } => Got::Failed(failed.failure()),
        Opened::Bytes(0) => {
            file::close(fd);
            Got::Read(None)
        }
        // Longer than its kind's packet can be: what the heap budgets a body at.
        Opened::Bytes(n) if stream.and_then(Stream::max_body).is_some_and(|max| n > max) => {
            file::close(fd);
            Got::Failed(Failure::from(Errno::EFBIG))
        }
        Opened::Bytes(n) => {
            let got = match body::take(stream, n) {
                Ok(block) => {
                    // SAFETY: a block just acquired, `n` long, and no one else's;
                    // the arrival holds it until `fetch_task` waits out the turn.
                    unsafe {
                        file::read(
                            fd,
                            core::slice::from_raw_parts_mut(mem::at::<u8>(block.0), n as usize),
                        );
                    };
                    Got::Read(Some(block))
                }
                Err(refused) => Got::Refused(refused),
            };
            file::close(fd);
            got
        }
    };
    arrived().push(Arrival {
        key,
        got,
        net_ms: now() - at_ms,
    });
    arrivals().notify();
}

/// Arrivals are handed to their owners on a later turn, once their bytes
/// are in, yielding where the turn is spent; the owner decodes them
/// (domain/view/demand.rs `admit_arrived`).
pub async fn fetch_task() {
    loop {
        notified(arrivals).await;
        while !arrived().is_empty() {
            let received = core::mem::take(arrived());
            next_turn(world_edge).await;
            for arrival in received {
                yield_if_spent(world_edge).await;
                on_fetch_arrived(arrival);
            }
        }
    }
}

fn on_fetch_arrived(Arrival { key, got, net_ms }: Arrival) {
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
            // An asset loads through the loader (sys/io/loader.rs), never a fetch: nothing would take its answer.
            xpute_kit::ensure!(stream != Stream::asset, ENOTRECOVERABLE);
            ANSWERED.get(List::new).push(Answered {
                key,
                stream,
                subject,
                block,
                phases,
            });
            request_world_tick();
            return;
        }
        // Only an absence says there is none; an empty read is not an object.
        Got::Read(None) => Failure::from(Errno::EBADMSG),
        Got::Failed(fail) => fail,
        Got::Refused(refused) => return settle(key, Ended::Refused(refused), Some(phases)),
    };
    settle(key, Ended::Failed(fail), Some(phases));
}

#[cfg(test)]
#[path = "fetch.test.rs"]
mod test;
