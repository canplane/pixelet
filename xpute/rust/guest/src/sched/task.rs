// xpute-guest/sched/task.rs

//! The guest's executor, installed once like a global allocator. An executor
//! may ignore priority, so nothing may rest on poll order: work ordered
//! within a turn runs in one task.

use core::future::Future;
use core::pin::Pin;

use crate::global::Global;

pub type Task = Pin<&'static mut (dyn Future<Output = ()> + 'static)>;

/// Lower is sooner.
pub type Priority = u8;

pub trait Executor {
    /// False when it has no room, keeping nothing of `task`.
    fn spawn(&self, task: Task, priority: Priority) -> bool;
    /// Never from inside a task.
    fn poll(&self);
    /// A task was queued since the last poll began.
    fn pending(&self) -> bool;
}

static EXECUTOR: Global<Option<&'static dyn Executor>> = Global::new();

fn executor() -> Option<&'static dyn Executor> {
    *EXECUTOR.get(|| None)
}

pub fn set(executor: &'static dyn Executor) {
    EXECUTOR.set(Some(executor));
}

pub fn spawn(future: impl Future<Output = ()> + 'static, priority: Priority) -> bool {
    let Some(e) = executor() else { return false };
    let raw: *mut (dyn Future<Output = ()> + 'static) = Box::into_raw(Box::new(future));
    // SAFETY: the box just made, handed out once.
    let spawned = e.spawn(Pin::static_mut(unsafe { &mut *raw }), priority);
    if !spawned {
        // SAFETY: refused, so the executor kept nothing of it; the box is ours again.
        drop(unsafe { Box::from_raw(raw) });
    }
    spawned
}

pub fn poll() {
    if let Some(e) = executor() {
        e.poll();
    }
}

pub fn pending() -> bool {
    executor().is_some_and(|e| e.pending())
}
