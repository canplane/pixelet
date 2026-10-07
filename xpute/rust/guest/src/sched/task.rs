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
    /// False when it has no room.
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

pub fn set_executor(executor: &'static dyn Executor) {
    EXECUTOR.set(Some(executor));
}

pub fn spawn(future: impl Future<Output = ()> + 'static, priority: Priority) -> bool {
    let Some(e) = executor() else { return false };
    let task: &'static mut (dyn Future<Output = ()> + 'static) = Box::leak(Box::new(future));
    e.spawn(Pin::static_mut(task), priority)
}

pub fn poll() {
    if let Some(e) = executor() {
        e.poll();
    }
}

pub fn pending() -> bool {
    executor().is_some_and(|e| e.pending())
}
