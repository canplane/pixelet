// xpute-native-runtime/event_loop.rs

//! The guest thread's one queue and its loop, a plain task queue as a page's
//! event loop is. Any thread pushes a task (`post_task`): the UI's methods
//! (lib.rs `post`), the app's lifecycle, the answers of a plugin's threads. The loop waits until a task
//! comes or its earliest timer is due and runs it on the guest's thread,
//! entering the guest where it must (`turn`); the program runs the answers due
//! and hands the UI its frames between turns (lib.rs), as a page's runs a
//! task, its microtasks, then posts.

use core::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, VecDeque};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use xpute_link::Return;

/// A piece of work for the guest's thread.
pub type Task = Box<dyn FnOnce() + Send>;

struct Queue {
    work: Mutex<VecDeque<Task>>,
    came: Condvar,
}

static QUEUE: Queue = Queue {
    work: Mutex::new(VecDeque::new()),
    came: Condvar::new(),
};

/// What hears a job's result, as the thread that ran it hands it back.
type Then = Box<dyn FnOnce(Box<dyn Any + Send>)>;

thread_local! {
    /// The timers set, earliest first, and the answer each gives.
    static TIMERS: RefCell<BTreeMap<Timer, Return<()>>> = const { RefCell::new(BTreeMap::new()) };
    /// What hears each job's result on this thread, by the job's id.
    static THENS: RefCell<BTreeMap<u64, Then>> = const { RefCell::new(BTreeMap::new()) };
    /// The next timer's or job's count.
    static NEXT: Cell<u64> = const { Cell::new(0) };
}

fn next() -> u64 {
    NEXT.replace(NEXT.get() + 1)
}

/// Pushes `task` for the guest's thread, from any thread.
pub fn post_task(task: Task) {
    QUEUE
        .work
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push_back(task);
    QUEUE.came.notify_one();
}

/// A timer `after` set, when it is due and a count that keeps it apart, until
/// it has answered or is cleared.
pub type Timer = (Instant, u64);

/// Answers `task_return` once `d` has passed, on the guest's thread
/// (xpute-native-host clock.rs `wait_for`), and returns the timer that will,
/// as a page's `setTimeout` returns its id.
pub fn after(d: Duration, task_return: Return<()>) -> Timer {
    let timer = (Instant::now() + d, next());
    TIMERS.with_borrow_mut(|t| t.insert(timer, task_return));
    timer
}

/// Clears `timer` before it answers, as a page's `clearTimeout` does, and
/// hands back what it would have answered; none once it has answered.
pub fn clear(timer: Timer) -> Option<Return<()>> {
    TIMERS.with_borrow_mut(|t| t.remove(&timer))
}

/// Keeps `then` for the result of a job another thread runs, and answers the
/// job's id, which that thread hands back with the result (`done`), so that
/// only the id and the result cross and `then` stays on this thread.
pub fn job<T: 'static>(then: impl FnOnce(T) + 'static) -> u64 {
    let id = next();
    let then: Then = Box::new(move |r| then(*r.downcast::<T>().ok().or_bug(Errno::EPROTO)));
    THENS.with_borrow_mut(|j| j.insert(id, then));
    id
}

/// The result of the job `id`, from the thread that ran it, of the type its
/// `job` takes.
pub fn done<T: Send + 'static>(id: u64, result: T) {
    post_task(Box::new(move || {
        if let Some(then) = THENS.with_borrow_mut(|j| j.remove(&id)) {
            then(Box::new(result));
        }
    }));
}

/// One turn of the guest thread's loop: the next task, or the earliest timer
/// due, waited for, the timers due answered and the task run. A task's panic,
/// which the host's hook has reported and turned into a trap
/// (xpute-native-host stop.rs `hook`), ends the task and not the loop.
pub(crate) fn turn() {
    let task = wait();
    answer_timers();
    if let Some(task) = task {
        let _ = std::panic::catch_unwind(core::panic::AssertUnwindSafe(task));
    }
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

/// Answers every timer due, earliest first.
fn answer_timers() {
    let now = Instant::now();
    while let Some(task_return) = TIMERS.with_borrow_mut(|t| {
        let k = *t.keys().next().filter(|(at, _)| *at <= now)?;
        t.remove(&k)
    }) {
        task_return.answer(Ok(()));
    }
}
