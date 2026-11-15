// pxl-gpu-host/job.rs

//! A job another thread runs for what the host serves, its result handed back
//! to the guest's thread over its runtime's `post` (xpute-host runtime.rs):
//! only the job's id and its result cross, and what hears the result stays
//! on the guest's thread.
//!
//! It is the GPU's: on a device the GPU host runs its device and presents on
//! a thread of its own (native.rs `on_gpu` says why). That thread is the
//! GPU's; the way back to the guest's thread is the runtime's (`post`), which
//! this host takes from xpute-host and does not name the runtime for.

use core::any::Any;
use core::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::cell::RefCell;
use std::collections::BTreeMap;

use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use xpute_host::runtime::runtime;

/// What hears a job's result, as the thread that ran it hands it back.
type Then = Box<dyn FnOnce(Box<dyn Any + Send>)>;

thread_local! {
    /// What hears each job's result on this thread, by the job's id.
    static THENS: RefCell<BTreeMap<u64, Then>> = const { RefCell::new(BTreeMap::new()) };
}

/// The next job's id. An id needs only be unique, so one counter serves
/// every thread.
static NEXT: AtomicU64 = AtomicU64::new(0);

/// Keeps `then` for the result of a job another thread runs, and answers the
/// job's id, which that thread hands back with the result (`complete`), so that
/// only the id and the result cross and `then` stays on this thread.
pub(super) fn job<T: 'static>(then: impl FnOnce(T) + 'static) -> u64 {
    let id = NEXT.fetch_add(1, Relaxed);
    let then: Then = Box::new(move |r| then(*r.downcast::<T>().ok().or_bug(Errno::EPROTO)));
    THENS.with_borrow_mut(|j| j.insert(id, then));
    id
}

/// The result of the job `id`, from the thread that ran it, of the type its
/// `job` takes.
pub(super) fn complete<T: Send + 'static>(id: u64, result: T) {
    (runtime().post)(Box::new(move || {
        if let Some(then) = THENS.with_borrow_mut(|j| j.remove(&id)) {
            then(Box::new(result));
        }
    }));
}
