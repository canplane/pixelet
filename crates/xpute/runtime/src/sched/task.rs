// xpute-runtime/sched/task.rs

//! The executor a guest's tasks run on, installed once, the way a global
//! allocator is: `Executor` is the interface, as `GlobalAlloc` is; the module
//! that assembles the guest installs one (`set_executor`), as
//! `#[global_allocator]` does; and code spawns through `spawn` without naming
//! it, as a `Vec` allocates. Which tasks there are is the code's own, not the
//! assembly's.
//!
//! A task reaches the executor with its type erased, as memory reaches an
//! allocator as a layout: a future kept for the program's life, its storage
//! taken from the heap once when it is spawned. What an executor does with it
//! — a pool, a queue, a priority — is its own.
//!
//! A task yields against the turn (sched/edge.rs), and the guest polls the
//! executor inside its turn; `pending` is how a turn that polled nothing
//! learns it still owes the tasks it woke a turn.
//!
//! **A task carries its priority from the start, and an executor may ignore
//! it.** One that orders its queue by it can be installed later without a
//! spawn changing, the way a layout carries its alignment to whichever
//! allocator. Until one does, nothing may rest on the order tasks are polled
//! in: work that has an order within a turn runs in one task, in that order.

use core::future::Future;
use core::pin::Pin;

use crate::global::Global;

/// A task: a future kept for the program's life.
pub type Task = Pin<&'static mut (dyn Future<Output = ()> + 'static)>;

/// How soon a task wants polling among those queued: lower sooner.
pub type Priority = u8;

pub trait Executor {
    /// Takes `task` to poll at `priority`; false when it has no room for it.
    fn spawn(&self, task: Task, priority: Priority) -> bool;
    /// Polls every task queued since the last poll. Never from inside a task.
    fn poll(&self);
    /// Whether a task was queued since the last poll began.
    fn pending(&self) -> bool;
}

static EXECUTOR: Global<Option<&'static dyn Executor>> = Global::new();

fn executor() -> Option<&'static dyn Executor> {
    *EXECUTOR.get(|| None)
}

/// Installs the executor, before anything spawns.
pub fn set_executor(executor: &'static dyn Executor) {
    EXECUTOR.set(Some(executor));
}

/// Spawns `future` at `priority` for the program's life. False when no
/// executor is installed or it has no room.
pub fn spawn(future: impl Future<Output = ()> + 'static, priority: Priority) -> bool {
    let Some(e) = executor() else { return false };
    let task: &'static mut (dyn Future<Output = ()> + 'static) = Box::leak(Box::new(future));
    e.spawn(Pin::static_mut(task), priority)
}

/// Polls the installed executor, if there is one.
pub fn poll() {
    if let Some(e) = executor() {
        e.poll();
    }
}

/// Whether the installed executor has a task queued and not yet polled.
pub fn pending() -> bool {
    executor().is_some_and(|e| e.pending())
}
