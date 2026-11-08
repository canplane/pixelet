// xpute-core/sched/task.rs

//! The core's executor is installed once, as a global allocator is. An
//! executor may ignore priority, so no correctness may rest on the order in
//! which tasks are polled. Work that must run in order within a turn runs in
//! one task.

use core::future::Future;
use core::pin::Pin;

use crate::global::Global;

pub type Task = Pin<&'static mut (dyn Future<Output = ()> + 'static)>;

/// A lower number runs sooner.
pub type Priority = u8;

pub trait Executor {
    /// Answers false when the executor has no room. It then keeps nothing of
    /// `task`, and the caller frees it.
    fn spawn(&self, task: Task, priority: Priority) -> bool;
    /// Must not be called from inside a task.
    fn poll(&self);
    /// Whether a task has been queued since the last poll began.
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
