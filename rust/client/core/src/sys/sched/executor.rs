// pxl-core/sys/sched/executor.rs

//! embassy's raw executor, which keeps the queue of the core's tasks; each
//! task is one of embassy's static tasks, spawned through `spawner`, and checks
//! the entry's budget at its own yield points (sys/sched.rs). Every entry's
//! end polls the executor until no task is ready (link.rs `end`). The pender
//! only records that work is queued, because embassy forbids calling poll from
//! it, and `pending` reports that record.

use embassy_executor::raw::Executor as Raw;
use embassy_executor::Spawner;
use pxl_global::Global;

struct OneThread;

critical_section::set_impl!(OneThread);

// SAFETY: the module runs on one thread and an entry never re-enters another.
unsafe impl critical_section::Impl for OneThread {
    unsafe fn acquire() {}

    unsafe fn release(_: ()) {}
}

static RAW: Global<Raw> = Global::new();

static PENDED: Global<bool> = Global::new();

#[unsafe(export_name = "__pender")]
fn pender(_: *mut ()) {
    PENDED.set(true);
}

fn raw() -> &'static Raw {
    RAW.get(|| Raw::new(core::ptr::null_mut()))
}

/// What spawns the core's tasks.
pub fn spawner() -> Spawner {
    raw().spawner()
}

/// Polls every task that is queued.
pub fn poll() {
    PENDED.set(false);
    // SAFETY: called from an entry's end alone (link.rs `end`), which no entry
    // re-enters, and never from the pender.
    unsafe { raw().poll() };
}

/// Whether a task has been queued since the last poll began.
pub fn pending() -> bool {
    *PENDED.get(|| false)
}
