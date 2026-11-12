// pxl-core/sys/io/fetch.rs

//! The world's reads: a tile after its bake's token by its quadkey, and a
//! shard's objects and the objects' top after OBJECT (sys/io/file.rs). A body
//! is read into a packet (sys/io/body.rs) and answered to the layer that
//! asked, and so is an absence. The layer decides what an absence means and
//! settles the fetch, and what it does with the packet is its own.

use xpute_core::clock::now;
use xpute_core::global::Global;
use xpute_core::io::fs::Fd;
use xpute_kit::collection::list::List;
use xpute_kit::status::errno::Errno;

use crate::sys::io::body::{self, Packet};
use crate::sys::io::demand::{PhaseList, Stream, TaskPhaseName};
use crate::sys::io::failure::{exit_of, Exit, Failure};
use crate::sys::io::file::{self, OpenResult, Owner, Request};
use crate::sys::io::param::MAX_CREDITS;
use crate::sys::io::request::{attempt_of, settle};
use crate::sys::sched::invalidate;

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

/// The answers, oldest first, past `drop_stopped`. Each is a live attempt's,
/// on a credit.
struct Queue {
    answers: List<Response, CREDITS>,
}

static QUEUE: Global<Queue> = Global::new();

fn queue() -> &'static mut Queue {
    QUEUE.get(|| Queue {
        answers: List::new(),
    })
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

/// Answers an attempt's end to its layer, and asks for a frame to take it;
/// any other end settles the attempt here. One the scheduler no longer holds
/// is dropped, and its body given back.
fn arrive(key: u64, at_ms: f64, body: Result<Option<Packet>, Exit>) {
    drop_stopped();
    let Some((stream, id)) = attempt_of(key) else {
        return;
    };
    let mut phases = PhaseList::default();
    phases.push(TaskPhaseName::net_ms, now() - at_ms);
    match body {
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
            invalidate();
        }
        Err(exit) => settle(key, exit, Some(phases)),
    }
}

/// Gives back the bodies of what the scheduler no longer holds.
fn drop_stopped() {
    queue().answers.retain(|a| attempt_of(a.key).is_some());
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
