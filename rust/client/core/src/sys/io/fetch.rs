// pxl-core/sys/io/fetch.rs

//! The world's reads: a tile below its bake's root by its quadkey, a shard's
//! objects and their top below OBJECT (sys/io/file.rs). A body is read into
//! a block (sys/io/body.rs) and answered, as an absence is, to the layer
//! that asked, whose it is to say what an absence means, to settle the fetch
//! and to release the block or keep it.

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
use crate::sys::io::demand::{Phases, Stream, TaskPhaseName};
use crate::sys::io::failure::{Exit, Failure};
use crate::sys::io::file::{self, Opened, Owner, Path};
use crate::sys::io::param::MAX_CREDITS;
use crate::sys::io::request::{attempt_of, settle};
use crate::sys::sched::edge::{request_world_tick, world_edge};

/// An open answered, until its body is filled.
struct Arrival {
    key: u64,
    /// Its body, none where the source has no object, or how it ended
    /// without one.
    got: Result<Option<Block>, Exit>,
    /// The read that fills the body, the runtime's until its head has passed
    /// it.
    read: Option<Pos>,
    net_ms: f64,
}

/// A body, or the source's absence, until the layer that asked takes it.
pub struct Response {
    pub key: u64,
    pub stream: Stream,
    pub id: u64,
    /// None where the source has no object for it.
    pub block: Option<Block>,
    pub phases: Phases,
}

const CREDITS: usize = MAX_CREDITS as usize;

/// No more of either than fetches are out, each on its credit; oldest first.
struct Queue {
    arrivals: List<Arrival, CREDITS>,
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

/// The open answered: a body's block taken and its read posted, the
/// descriptor closed either way.
pub fn on_opened(fd: Fd, key: u64, at_ms: f64, opened: Opened) {
    let stream = attempt_of(key).map(|(stream, _)| stream);
    let mut read = None;
    let got = match opened {
        Opened::Absent => Ok(None),
        Opened::Failed { .. } => Err(Exit::Failed(Failure::from(opened))),
        // Only an absence says there is none: an empty file is no object.
        Opened::Bytes(0) => {
            file::close(fd);
            Err(Exit::Failed(Failure::from(Errno::EBADMSG)))
        }
        // Longer than its kind's packet can be.
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

/// Each arrival handed on once its body is filled, oldest first, yielding
/// where the turn is spent; the layer decodes it (domain/view/demand.rs
/// `admit_arrived`).
pub async fn fetch_task() {
    loop {
        notified(notify).await;
        while !queue().arrivals.is_empty() {
            yield_if_spent(world_edge).await;
            let filled = queue().arrivals.iter().position(Arrival::is_filled);
            match filled {
                Some(at) => arrived(queue().arrivals.remove(at)),
                None => runtime_turn(world_edge).await,
            }
        }
    }
}

/// A body or an absence answered, and a turn asked to take it; anything
/// else settled.
fn arrived(a: Arrival) {
    let key = a.key;
    // A key the requests do not hold is one a reset forgot.
    let Some((stream, id)) = attempt_of(key) else {
        return release(a.got.ok().flatten());
    };
    let mut phases = Phases::default();
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
        Err(ended) => settle(key, ended, Some(phases)),
    }
}

/// How many answers to one of `of` wait to be taken.
pub fn answer_cnt(of: &[Stream]) -> usize {
    let answers = &queue().answers;
    answers.iter().filter(|a| of.contains(&a.stream)).count()
}

/// The oldest answer to one of `of`.
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
