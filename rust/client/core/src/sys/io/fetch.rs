// pxl-core/sys/io/fetch.rs

//! The world's reads: a tile after its bake's token by its quadkey, and a
//! shard's objects and the objects' top after OBJECT (sys/io/file.rs), each by
//! a task of its own. A body is read whole and answered to the layer that
//! asked, and so is an absence. The layer decides what an absence means and
//! settles the fetch, and what it does with the packet is its own.

use pxl_global::Global;
use xpute_guest::clock::now;
use xpute_kit::collection::list::List;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;
use xpute_link::abi::types::Token;

use crate::spec::manifest::mem;
use crate::sys::io::body::{self, Body};
use crate::sys::io::demand::{PhaseList, Stream, TaskPhaseName};
use crate::sys::io::failure::{exit_of, Exit, Failure};
use crate::sys::io::file;
use crate::sys::io::param::CREDIT_CAP;
use crate::sys::io::request::{attempt_of, settle};
use crate::sys::io::room::Refusal;
use crate::sys::sched::{executor, invalidate};

pub(crate) struct Response {
    pub key: u64,
    pub stream: Stream,
    pub id: u64,
    /// None where the source has no object for it.
    pub pkt: Option<Body>,
    pub phases: PhaseList,
}

/// The answers waiting for their layers to take them.
static ANSWERS: Global<List<Response, { CREDIT_CAP as usize }>> = Global::new();

fn answers() -> &'static mut List<Response, { CREDIT_CAP as usize }> {
    ANSWERS.get(List::new)
}

/// Reads the want `id` of `stream` for the attempt `key`, in a task of its
/// own. The pool is the credits, which already bound the attempts in flight.
pub(crate) fn start(key: u64, stream: Stream, id: u64) {
    let Some(path) = file::path_of(stream, id) else {
        return;
    };
    executor::spawner().spawn(fetch(key, stream, path, now()).or_bug(Errno::ENOSPC));
}

/// Reads a body whole, into a body slot where its stream takes one, and
/// answers it. Only an absent file means there is no object, so an empty file
/// is a bad body, and one longer than a packet of its kind can be is refused.
/// With every slot out the read waits for one given back, before it opens
/// the file. A canceled read ends aborted (`exit_of`), so its want starts
/// again, and one its stop canceled reaches no one.
#[embassy_executor::task(pool_size = CREDIT_CAP as usize)]
async fn fetch(key: u64, stream: Stream, path: Vec<Token>, at_ms: f64) {
    let most = stream.max_body().unwrap_or(mem::MIN_HEAP_SZ);
    let read = if stream.row().slotted {
        let Some(mut slot) = body::take() else {
            return arrive(key, at_ms, Err(Exit::Refused(Refusal::bodies())));
        };
        let take = move |n| {
            slot.set_len(n);
            Ok(slot)
        };
        file::read_whole(&path, most, Some(key), take)
            .await
            .map(Body::Slot)
    } else {
        let heap = file::read_whole(&path, most, Some(key), file::heap).await;
        heap.map(|v| Body::Heap(v.into_boxed_slice()))
    };
    let body = match read {
        Ok(b) if b.is_empty() => Err(exit_of(Failure::from(Errno::EBADMSG))),
        Ok(b) => Ok(Some(b)),
        Err(Errno::ENOENT) => Ok(None),
        Err(e) => Err(exit_of(Failure::from(e))),
    };
    arrive(key, at_ms, body);
}

/// Answers an attempt's end to its layer, and asks for a frame to take it;
/// any other end settles the attempt here. One the scheduler no longer holds
/// is dropped, and its body given back.
fn arrive(key: u64, at_ms: f64, body: Result<Option<Body>, Exit>) {
    drop_stopped();
    let Some((stream, id)) = attempt_of(key) else {
        return;
    };
    let mut phases = PhaseList::default();
    phases.push(TaskPhaseName::NetMs, now() - at_ms);
    match body {
        Ok(pkt) => {
            if let Some(pkt) = &pkt {
                phases.push(TaskPhaseName::Bytes, pkt.len() as f64);
            }
            answers().push(Response {
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
    answers().retain(|a| attempt_of(a.key).is_some());
}

/// How many answers to any stream in `of` wait to be taken.
pub(crate) fn answer_cnt(of: &[Stream]) -> usize {
    answers().iter().filter(|a| of.contains(&a.stream)).count()
}

/// The oldest answer to any stream in `of`.
pub(crate) fn take_answer(of: &[Stream]) -> Option<Response> {
    let a = answers();
    let at = a.iter().position(|a| of.contains(&a.stream))?;
    Some(a.remove(at))
}
