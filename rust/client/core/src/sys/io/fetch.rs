// pxl-core/sys/io/fetch.rs

//! The world's reads: a tile after its bake's token by its quadkey, and a
//! shard's objects and the objects' top after OBJECT (sys/io/file.rs). A body
//! is read into a packet (sys/io/body.rs) and answered to the layer that
//! asked, and so is an absence. The layer decides what an absence means and
//! settles the fetch, and what it does with the packet is its own.

use xpute_core::clock::now;
use xpute_core::global::Global;
use xpute_core::io::fs::Fd;
use xpute_core::sched::turn::{notified, yield_if_spent, Notify};
use xpute_kit::collection::list::List;
use xpute_kit::status::errno::Errno;

use crate::sys::io::body::{self, Packet};
use crate::sys::io::demand::{PhaseList, Stream, TaskPhaseName};
use crate::sys::io::failure::{exit_of, Exit, Failure};
use crate::sys::io::file::{self, OpenResult, Owner, Request};
use crate::sys::io::param::MAX_CREDITS;
use crate::sys::io::request::{attempt_of, settle};
use crate::sys::sched::{self, request_world_tick};

/// A fetch's end, held until the turn hands it on: its body, filled, or None
/// where the source has no object; or how the attempt ended without one.
struct Arrival {
    key: u64,
    body: Result<Option<Packet>, Exit>,
    net_ms: f64,
}

/// A body, or the source's absence, held until the layer that asked takes
/// it.
pub struct Response {
    pub key: u64,
    pub stream: Stream,
    pub id: u64,
    /// None where the source has no object for it.
    pub pkt: Option<Packet>,
    pub phases: PhaseList,
}

const CREDITS: usize = MAX_CREDITS as usize;

/// The arrivals and the answers, oldest first, past `drop_stopped`. Each is
/// a live attempt's, on a credit.
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

/// Opens the file of `stream`'s want `id`.
pub fn start(key: u64, stream: Stream, id: u64) {
    let owner = Owner::Fetch { key, at_ms: now() };
    file::with_path(stream, id, |path| file::open(path, owner));
}

/// Takes the answer to a fetch's open. For a body, a block is taken, and the
/// body is read into it and arrives as its close is completed (`on_read`).
/// Any other answer arrives now, and the descriptor is closed.
pub fn on_opened(fd: Fd, key: u64, at_ms: f64, opened: OpenResult) {
    let stream = attempt_of(key).map(|(stream, _)| stream);
    let body = match opened {
        OpenResult::Absent => Ok(None),
        OpenResult::Failed { .. } => Err(exit_of(Failure::from(opened))),
        // Only an absent file means there is no object, so an empty file is
        // a bad body.
        OpenResult::Bytes(0) => {
            file::close(fd);
            Err(Exit::Failed(Failure::from(Errno::EBADMSG)))
        }
        // The body is longer than a packet of its kind can be.
        OpenResult::Bytes(n) if stream.and_then(Stream::max_body).is_some_and(|max| n > max) => {
            file::close(fd);
            Err(Exit::Failed(Failure::from(Errno::EFBIG)))
        }
        OpenResult::Bytes(n) => match body::take(stream, n) {
            Ok(pkt) => return file::read(fd, pkt, Request::Fetch { key, at_ms }),
            Err(refused) => {
                file::close(fd);
                Err(Exit::Refused(refused))
            }
        },
    };
    arrive(key, at_ms, body);
}

/// The read of a fetch's body is done, as its close is completed: the packet,
/// filled, or, where the close failed, the errno, and the attempt failed.
pub fn on_read(key: u64, at_ms: f64, read: Result<Packet, Errno>) {
    let body = read
        .map(Some)
        .map_err(|errnum| exit_of(Failure::from(errnum)));
    arrive(key, at_ms, body);
}

/// Queues an attempt's end for the turn to hand on; one the scheduler no
/// longer holds is dropped, and its body given back.
fn arrive(key: u64, at_ms: f64, body: Result<Option<Packet>, Exit>) {
    drop_stopped();
    if attempt_of(key).is_none() {
        return;
    }
    let q = queue();
    q.arrivals.push(Arrival {
        key,
        body,
        net_ms: now() - at_ms,
    });
    q.notify.notify();
}

/// Gives back the bodies of what the scheduler no longer holds. This asks
/// for no time of the turn, so it runs whether or not the turn is spent.
fn drop_stopped() {
    let q = queue();
    q.arrivals.retain(|a| attempt_of(a.key).is_some());
    q.answers.retain(|a| attempt_of(a.key).is_some());
}

/// Hands on each arrival, oldest first, and yields where the turn is spent.
/// The layer decodes what it is handed (domain/view/demand.rs
/// `admit_arrived`).
#[embassy_executor::task]
pub async fn fetch_task() {
    loop {
        notified(notify).await;
        while !queue().arrivals.is_empty() {
            yield_if_spent(sched::turn).await;
            drop_stopped();
            if !queue().arrivals.is_empty() {
                arrived(queue().arrivals.remove(0));
            }
        }
    }
}

/// Answers a body or an absence to its layer, and asks for a turn to take
/// it. Any other end settles the attempt here.
fn arrived(a: Arrival) {
    let key = a.key;
    let Some((stream, id)) = attempt_of(key) else {
        return;
    };
    let mut phases = PhaseList::default();
    phases.push(TaskPhaseName::net_ms, a.net_ms);
    match a.body {
        Ok(pkt) => {
            if let Some(pkt) = &pkt {
                phases.push(TaskPhaseName::bytes, pkt.len() as f64);
            }
            queue().answers.push(Response {
                key,
                stream,
                id,
                pkt,
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

#[cfg(test)]
#[path = "fetch.test.rs"]
mod test;
