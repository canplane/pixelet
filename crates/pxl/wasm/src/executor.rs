// pxl_wasm/executor.rs

//! The executor the module's tasks run on: embassy's raw executor, polled
//! only inside the doorbell's turn. A task is written against
//! xpute_core::sched::edge and nothing here, the way code allocates without
//! naming the slab this module installs; which executor polls it is this
//! module's choice alone.
//!
//! The executor tells its owner it has work through the pender, a function
//! it links to by name. Here that only records the fact: poll is never
//! called from it, which the executor forbids, and a task woken while the
//! turn polls asks for the next edge rather than running again past the
//! quota. A task the turn's rising edge woke is polled in that same turn.
//!
//! One thread, and no turn re-enters another, so the critical section the
//! executor takes has nothing to exclude.

use embassy_executor::raw::Executor;
use pxl_core::module::world::change::world_edge;
use pxl_core::sys::global::Global;

struct OneThread;

critical_section::set_impl!(OneThread);

// SAFETY: the module runs on one thread and a turn never re-enters another.
unsafe impl critical_section::Impl for OneThread {
    unsafe fn acquire() {}

    unsafe fn release(_: ()) {}
}

static EXECUTOR: Global<Executor> = Global::new();

/// Set by the pender: a task was queued since the last poll began.
static PENDED: Global<bool> = Global::new();

#[unsafe(export_name = "__pender")]
fn pender(_: *mut ()) {
    PENDED.set(true);
}

fn executor() -> &'static Executor {
    EXECUTOR.get(|| Executor::new(core::ptr::null_mut()))
}

/// Polls every task queued for this turn; one queued again while polling
/// asks for the next edge.
pub fn poll() {
    PENDED.set(false);
    // SAFETY: called from the doorbell alone, which no turn re-enters, and
    // never from the pender.
    unsafe { executor().poll() };
    if *PENDED.get(|| false) {
        world_edge().ask_next();
    }
}
