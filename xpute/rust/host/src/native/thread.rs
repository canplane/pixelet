// xpute-host/native/thread.rs

//! The guest's own thread: every call is handed to it while the caller waits,
//! through one handoff made at boot, which callers on several threads take in
//! turn. A hostcall the guest makes meanwhile is handed back to the caller,
//! which answers it on its own thread and waits again.

use core::cell::{Cell, RefCell};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};

use xpute_guest::global::on_marked_thread;

/// A caller's work, borrowed: the caller waits until it has run.
struct Job(*mut (dyn FnMut() + Send + 'static));

// SAFETY: the closure it points at is `Send`, and only the guest's thread
// runs it, while the caller that owns it waits.
unsafe impl Send for Job {}

/// What the guest's thread hands back while a caller waits.
enum Back {
    Done,
    /// A hostcall's record, at its offset, for the caller to answer.
    Hostcall(u32),
}

struct Handoff {
    jobs: SyncSender<Job>,
    back: Receiver<Back>,
    answers: SyncSender<i32>,
}

static HANDOFF: OnceLock<Mutex<Handoff>> = OnceLock::new();

/// The program's answer to a hostcall, run on the waiting caller's thread.
pub type Answer = extern "C" fn(at: u32) -> i32;

static ANSWER: Mutex<Option<Answer>> = Mutex::new(None);

/// `answer` answers the guest's hostcalls from the next on; none, ENOSYS.
pub fn answer_with(answer: Option<Answer>) {
    *ANSWER.lock().unwrap_or_else(PoisonError::into_inner) = answer;
}

fn answer(at: u32) -> i32 {
    let answer = *ANSWER.lock().unwrap_or_else(PoisonError::into_inner);
    answer.map_or(xpute_kit::status::errno::Errno::ENOSYS as i32, |f| f(at))
}

thread_local! {
    /// A `run` or `while_idle` is under way on this thread.
    #[cfg_attr(target_os = "android", allow(clippy::missing_const_for_thread_local, reason = "already const; the lint misreads Android's expansion"))]
    static HOLDING: Cell<bool> = const { Cell::new(false) };
    /// The guest's thread's end of the handoff, for its hostcalls.
    #[cfg_attr(target_os = "android", allow(clippy::missing_const_for_thread_local, reason = "already const; the lint misreads Android's expansion"))]
    static GUEST_END: RefCell<Option<(SyncSender<Back>, Receiver<i32>)>> = const { RefCell::new(None) };
}

/// The handoff, held, this thread its holder until it is let go.
struct Held(MutexGuard<'static, Handoff>);

impl Drop for Held {
    fn drop(&mut self) {
        HOLDING.with(|h| h.set(false));
    }
}

/// The handoff, refused where taking it would wait for itself: on the guest's
/// thread, or on one that holds it. A poisoned lock is still whole.
fn handoff() -> Held {
    assert!(
        !on_marked_thread(),
        "a call into the guest from its own thread would wait for itself"
    );
    assert!(
        !HOLDING.with(Cell::get),
        "a call into the guest inside another on the same thread would wait for itself"
    );
    let held = Held(
        HANDOFF
            .get()
            .expect("the guest is booted")
            .lock()
            .unwrap_or_else(PoisonError::into_inner),
    );
    HOLDING.with(|h| h.set(true));
    held
}

/// Starts the guest's thread and runs `first` there before anything else.
pub fn start(name: &'static str, stack_bytes: usize, first: impl FnOnce() + Send) {
    let (jobs, next) = sync_channel::<Job>(1);
    let (sent, back) = sync_channel::<Back>(1);
    let (answers, answered) = sync_channel::<i32>(1);
    std::thread::Builder::new()
        .name(name.into())
        .stack_size(stack_bytes)
        .spawn(move || {
            GUEST_END.with(|e| *e.borrow_mut() = Some((sent.clone(), answered)));
            for Job(job) in next {
                // SAFETY: the caller's closure, which it keeps until the
                // answer below has reached it.
                unsafe { (*job)() };
                let _ = sent.send(Back::Done);
            }
        })
        .expect("the system would not start the guest's thread");
    assert!(
        HANDOFF
            .set(Mutex::new(Handoff {
                jobs,
                back,
                answers
            }))
            .is_ok(),
        "the guest's thread is started once"
    );
    run(first);
}

/// The guest's hostcall, on its own thread inside a call: handed to the
/// caller waiting on it, which runs the program's answer.
pub fn hostcall(at: u32) -> i32 {
    GUEST_END.with(|e| {
        let e = e.borrow();
        let (sent, answered) = e
            .as_ref()
            .expect("a hostcall is made on the guest's thread");
        sent.send(Back::Hostcall(at)).expect("the caller is gone");
        answered.recv().expect("the caller is gone")
    })
}

/// `f` on the guest's thread, the caller waiting; each hostcall it makes
/// meanwhile answered on the caller's.
pub fn run<R: Send>(f: impl FnOnce() -> R + Send) -> R {
    let h = handoff();
    let mut f = Some(f);
    let mut out = None;
    let mut run = || out = f.take().map(|f| f());
    let run: *mut (dyn FnMut() + Send + '_) = &mut run;
    // SAFETY: the borrow is made `'static` only to cross to the guest's
    // thread; this call waits for the run to finish before `run` goes.
    let run: *mut (dyn FnMut() + Send + 'static) = unsafe { core::mem::transmute(run) };
    h.0.jobs.send(Job(run)).expect("the guest's thread is gone");
    while let Back::Hostcall(at) = h.0.back.recv().expect("the guest's thread is gone") {
        h.0.answers
            .send(answer(at))
            .expect("the guest's thread is gone");
    }
    drop(h);
    out.expect("the guest's thread ran the call")
}

/// `f` on this thread, the handoff held so the guest's runs nothing meanwhile.
pub fn while_idle<R>(f: impl FnOnce() -> R) -> R {
    let _h = handoff();
    f()
}
