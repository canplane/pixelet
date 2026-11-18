// xpute-runtime-native/event_loop.rs

//! The guest thread's one queue and its loop, a plain task queue as the web's
//! event loop is, and its timers: the host's `post`, `after` and `clear`
//! (xpute-host runtime.rs). Any thread pushes a task (`post_task`): the
//! UI's methods (lib.rs `post`), the app's lifecycle, the answers of a
//! plugin's threads. The loop waits until a task comes or its earliest timer
//! is due and runs it on the guest's thread, entering the guest where it must
//! (`turn`); the program runs the answers due and hands the UI its frames
//! between turns (native.rs `run`), as the web's runs a task, its microtasks,
//! then posts.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, VecDeque};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

use xpute_host::runtime::{Task, Timer};
use xpute_link::Return;

struct Queue {
    work: Mutex<VecDeque<Task>>,
    came: Condvar,
}

static QUEUE: Queue = Queue {
    work: Mutex::new(VecDeque::new()),
    came: Condvar::new(),
};

thread_local! {
    /// The timers set, earliest first, and the answer each gives.
    static TIMERS: RefCell<BTreeMap<Timer, Return<()>>> = const { RefCell::new(BTreeMap::new()) };
    /// The next timer's count.
    static NEXT: Cell<u64> = const { Cell::new(0) };
}

/// Pushes `task` for the guest's thread, from any thread.
pub(crate) fn post_task(task: Task) {
    QUEUE
        .work
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push_back(task);
    QUEUE.came.notify_one();
}

/// Answers `task_return` once `d` has passed, on the guest's thread
/// (xpute-host clock.rs `wait_for`), and returns the timer that will,
/// as the web's `setTimeout` returns its id.
pub(crate) fn after(d: Duration, task_return: Return<()>) -> Timer {
    let timer = (Instant::now() + d, NEXT.replace(NEXT.get() + 1));
    TIMERS.with_borrow_mut(|t| t.insert(timer, task_return));
    timer
}

/// Clears `timer` before it answers, as the web's `clearTimeout` does, and
/// hands back what it would have answered; none once it has answered.
pub(crate) fn clear(timer: Timer) -> Option<Return<()>> {
    TIMERS.with_borrow_mut(|t| t.remove(&timer))
}

/// One turn of the guest thread's loop: the next task, or the earliest timer
/// due, waited for, the task run and the timers that were due before it
/// answered. A timer the task sets is answered in a later turn, a wait of 0
/// at the next (wit/clocks/monotonic-clock.wit). A panic ends the turn alone
/// (native.rs `run`): the task taken has run by then, and a timer not yet
/// answered stays for the next turn.
pub(crate) fn turn() {
    let task = wait();
    let now = Instant::now();
    if let Some(task) = task {
        task();
    }
    answer_timers(now);
}

/// The next task, waited for until the earliest timer is due: None when that
/// timer is due first.
fn wait() -> Option<Task> {
    let until = TIMERS.with_borrow(|t| t.keys().next().map(|(at, _)| *at));
    let mut q = QUEUE.work.lock().unwrap_or_else(|e| e.into_inner());
    loop {
        if let Some(w) = q.pop_front() {
            return Some(w);
        }
        q = match until {
            None => QUEUE.came.wait(q).unwrap_or_else(|e| e.into_inner()),
            Some(at) => {
                let now = Instant::now();
                if now >= at {
                    return None;
                }
                QUEUE
                    .came
                    .wait_timeout(q, at - now)
                    .unwrap_or_else(|e| e.into_inner())
                    .0
            }
        };
    }
}

/// Answers every timer due at `now`, earliest first.
fn answer_timers(now: Instant) {
    while let Some(task_return) = TIMERS.with_borrow_mut(|t| {
        let k = *t.keys().next().filter(|(at, _)| *at <= now)?;
        t.remove(&k)
    }) {
        task_return.answer(Ok(()));
    }
}
