// pxl-core/sys/io/fetch.rs

//! The world's reads: a tile below its bake's root by its quadkey, and a
//! shard's objects and the objects' top below OBJECT (sys/io/file.rs). A body
//! is read into a block (sys/io/body.rs) and answered to the layer that
//! asked, and so is an absence. The layer decides what an absence means,
//! settles the fetch, and releases the block or keeps it.

use xpute_core::clock::now;
use xpute_core::global::Global;
use xpute_core::ipc::door::Pos;
use xpute_core::ipc::fs::Fd;
use xpute_core::mem;
use xpute_core::mem::heap::Block;
use xpute_core::sched::edge::{notified, runtime_turn, yield_if_spent, Notify};
use xpute_kit::collection::list::List;
use xpute_kit::status::errno::Errno;

use crate::sys::io::body;
use crate::sys::io::demand::{PhaseList, Stream, TaskPhaseName};
use crate::sys::io::failure::{Exit, Failure};
use crate::sys::io::file::{self, Opened, Owner, Path};
use crate::sys::io::param::MAX_CREDITS;
use crate::sys::io::request::{attempt_of, settle};
use crate::sys::sched::edge::{request_world_tick, world_edge};

/// An answered open, held until its body is filled.
struct Arrival {
    key: u64,
    /// The body, or None where the source has no object; or how the attempt
    /// ended without a body.
    got: Result<Option<Block>, Exit>,
    /// The read that fills the body. The body belongs to the runtime until
    /// the runtime's head has passed the read.
    read: Option<Pos>,
    net_ms: f64,
}

/// A body, or the source's absence, held until the layer that asked takes
/// it.
pub struct Response {
    pub key: u64,
    pub stream: Stream,
    pub id: u64,
    /// None where the source has no object for it.
    pub block: Option<Block>,
    pub phases: PhaseList,
}

const CREDITS: usize = MAX_CREDITS as usize;

/// The arrivals and the answers, oldest first, past `drop_stopped`. A live
/// arrival or answer is on a credit. A stopped arrival stays until the runtime
/// has passed its read. The rings are read in order, so an open answered after
/// a read was posted was posted before it: the arrivals whose reads are not
/// passed were all out at once, and are no more than the credits.
struct Queue {
    arrivals: List<Arrival, { 2 * CREDITS }>,
    answers: List<Response, CREDITS>,
    notify: Notify,
}

static QUEUE: Global<Queue> = Global::new();

fn queue() -> &'static mut Queue {
    QUEUE.get(|| Queue {
        arrivals: List::new(),
        answers: List::new(),
        notify: Notify::new(),
    })
}

fn notify() -> &'static mut Notify {
    &mut queue().notify
}

impl Arrival {
    fn is_filled(&self) -> bool {
        self.read.is_none_or(file::is_consumed)
    }
}

pub fn start(key: u64, stream: Stream, id: u64) {
    let Some(root) = stream.root() else { return };
    let owner = Owner::Fetch { key, at_ms: now() };
    file::open(root, Path::of_quadrant(id), owner);
}

/// Takes the answer to a fetch's open. For a body, a block is taken and its
/// read is posted. The descriptor is closed either way.
pub fn on_opened(fd: Fd, key: u64, at_ms: f64, opened: Opened) {
    drop_stopped();
    let stream = attempt_of(key).map(|(stream, _)| stream);
    let mut read = None;
    let got = match opened {
        Opened::Absent => Ok(None),
        Opened::Failed { .. } => Err(Exit::Failed(Failure::from(opened))),
        // Only an absent file means there is no object, so an empty file is
        // a bad body.
        Opened::Bytes(0) => {
            file::close(fd);
            Err(Exit::Failed(Failure::from(Errno::EBADMSG)))
        }
        // The body is longer than a packet of its kind can be.
        Opened::Bytes(n) if stream.and_then(Stream::max_body).is_some_and(|max| n > max) => {
            file::close(fd);
            Err(Exit::Failed(Failure::from(Errno::EFBIG)))
        }
        Opened::Bytes(n) => {
            let got = match body::take(stream, n) {
                Ok(block) => {
                    // SAFETY: a block just taken, `n` long and no one else's,
                    // held unread by its arrival until the runtime's head has
                    // passed the read.
                    read = Some(unsafe {
                        file::read(
                            fd,
                            core::slice::from_raw_parts_mut(mem::at::<u8>(block.0), n as usize),
                        )
                    });
                    Ok(Some(block))
                }
                Err(refused) => Err(Exit::Refused(refused)),
            };
            file::close(fd);
            got
        }
    };
    let q = queue();
    q.arrivals.push(Arrival {
        key,
        got,
        read,
        net_ms: now() - at_ms,
    });
    q.notify.notify();
}

/// Gives back the bodies of what the scheduler no longer holds: an answer, and
/// an arrival once the runtime has passed its read. This asks for no time of
/// the turn, so it runs whether or not the turn is spent.
fn drop_stopped() {
    let q = queue();
    q.arrivals.retain(|a| {
        let stopped = a.is_filled() && attempt_of(a.key).is_none();
        if stopped {
            release(a.got.ok().flatten());
        }
        !stopped
    });
    q.answers.retain(|a| {
        let stopped = attempt_of(a.key).is_none();
        if stopped {
            release(a.block);
        }
        !stopped
    });
}

/// Hands on each arrival once its body is filled, oldest first, and yields
/// where the turn is spent. The layer decodes what it is handed
/// (domain/view/demand.rs `admit_arrived`).
pub async fn fetch_task() {
    loop {
        notified(notify).await;
        while !queue().arrivals.is_empty() {
            yield_if_spent(world_edge).await;
            drop_stopped();
            let filled = queue().arrivals.iter().position(Arrival::is_filled);
            match filled {
                Some(at) => arrived(queue().arrivals.remove(at)),
                None if queue().arrivals.is_empty() => {}
                None => runtime_turn(world_edge).await,
            }
        }
    }
}

/// Answers a body or an absence to its layer, and asks for a turn to take
/// it. Any other end settles the attempt here.
fn arrived(a: Arrival) {
    let key = a.key;
    // A runtime on its own thread may pass a stopped arrival's read after
    // `drop_stopped` looked, so an attempt that has stopped still comes here.
    let Some((stream, id)) = attempt_of(key) else {
        return release(a.got.ok().flatten());
    };
    let mut phases = PhaseList::default();
    phases.push(TaskPhaseName::net_ms, a.net_ms);
    match a.got {
        Ok(block) => {
            if let Some((_, bytes)) = block {
                phases.push(TaskPhaseName::bytes, f64::from(bytes));
            }
            queue().answers.push(Response {
                key,
                stream,
                id,
                block,
                phases,
            });
            request_world_tick();
        }
        Err(exit) => settle(key, exit, Some(phases)),
    }
}

/// How many answers to any stream in `of` wait to be taken.
pub fn answer_cnt(of: &[Stream]) -> usize {
    let answers = &queue().answers;
    answers.iter().filter(|a| of.contains(&a.stream)).count()
}

/// The oldest answer to any stream in `of`.
pub fn take_answer(of: &[Stream]) -> Option<Response> {
    let answers = &mut queue().answers;
    let at = answers.iter().position(|a| of.contains(&a.stream))?;
    Some(answers.remove(at))
}

pub fn body<'a>((at, bytes): Block) -> &'a [u8] {
    // SAFETY: the block `on_opened` took for this body alone, `bytes` long,
    // not released while it is read.
    unsafe { core::slice::from_raw_parts(mem::at::<u8>(at), bytes as usize) }
}

pub fn release(block: Option<Block>) {
    if let Some(block) = block {
        // SAFETY: a body's block, taken by `on_opened` and released once.
        unsafe { body::give(block) };
    }
}

#[cfg(test)]
#[path = "fetch.test.rs"]
mod test;
