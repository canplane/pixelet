// xpute-host/native/thread.rs

//! The guest's own thread, started at boot. Every call into the guest is
//! handed to it, and the caller waits: one of the two runs at a time, and
//! the guest is single-threaded on its thread rather than on its callers'.
//!
//! The handoff is made once and used for every call: a turn a frame, and a
//! call that made a job and a channel for its answer each time allocated
//! twice a frame for nothing. Callers may be several threads, so they take
//! the handoff in turn.

use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};

use xpute_guest::global::on_marked_thread;

/// A caller's work, borrowed: the caller waits until it has run, so what it
/// points at outlives the run.
struct Job(*mut (dyn FnMut() + Send + 'static));

// SAFETY: the closure it points at is `Send`, and only the guest's thread
// runs it, while the caller that owns it waits.
unsafe impl Send for Job {}

struct Handoff {
    jobs: SyncSender<Job>,
    done: Receiver<()>,
}

static HANDOFF: OnceLock<Mutex<Handoff>> = OnceLock::new();

/// The handoff, held. A caller that panicked holding it left it as it was:
/// the guest's thread had run its job or never got it.
fn handoff() -> MutexGuard<'static, Handoff> {
    HANDOFF.get().expect("the guest is booted").lock().unwrap_or_else(PoisonError::into_inner)
}

/// Starts the guest's thread, named `name` with a stack of `stack_bytes`,
/// and runs `first` there before anything else reaches it.
pub fn start(name: &'static str, stack_bytes: usize, first: impl FnOnce() + Send) {
    let (jobs, next) = sync_channel::<Job>(1);
    let (finished, done) = sync_channel::<()>(1);
    std::thread::Builder::new()
        .name(name.into())
        .stack_size(stack_bytes)
        .spawn(move || {
            for Job(job) in next {
                // SAFETY: the caller's closure, which it keeps until the
                // answer below has reached it.
                unsafe { (*job)() };
                let _ = finished.send(());
            }
        })
        .expect("the system would not start the guest's thread");
    assert!(HANDOFF.set(Mutex::new(Handoff { jobs, done })).is_ok(), "the guest's thread is started once");
    run(first);
}

/// Runs `f` on the guest's thread and returns what it returns once it has.
pub fn run<R: Send>(f: impl FnOnce() -> R + Send) -> R {
    assert!(!on_marked_thread(), "a call into the guest from its own thread would wait for itself");
    let h = handoff();
    let mut f = Some(f);
    let mut out = None;
    let mut run = || out = f.take().map(|f| f());
    let run: *mut (dyn FnMut() + Send + '_) = &mut run;
    // SAFETY: the borrow is made `'static` only to cross to the guest's
    // thread; this call waits for the run to finish before `run` goes.
    let run: *mut (dyn FnMut() + Send + 'static) = unsafe { core::mem::transmute(run) };
    h.jobs.send(Job(run)).expect("the guest's thread is gone");
    h.done.recv().expect("the guest's thread is gone");
    drop(h);
    out.expect("the guest's thread ran the call")
}

/// Runs `f` on this thread while the guest's runs nothing: the handoff is
/// held, so no call is on the guest's thread, and `f` is the only one
/// touching the guest.
pub fn while_idle<R>(f: impl FnOnce() -> R) -> R {
    let _h = handoff();
    f()
}
