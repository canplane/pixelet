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
//! quota. A task the turn's rising edge woke is polled in that same turn's
//! frame; a turn with no frame, which runs no task, asks for the next one.
//!
//! The tasks are pxl-core's own `async fn`s, each given a pool of one here: the
//! executor's task attribute is this module's, so pxl-core never names it.
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

#[embassy_executor::task]
async fn fetches() {
    pxl_core::module::world::fetch::fetch_task().await
}

#[embassy_executor::task]
async fn builds() {
    pxl_core::module::world::chunk_stage::build_task().await
}

#[embassy_executor::task]
async fn models() {
    pxl_core::module::world::gltf_cache::gltf_task().await
}

/// Spawns the module's tasks, once, before the first turn. Each has a pool of
/// one and is spawned only here, so a spawn is never refused.
pub fn spawn() {
    let spawner = executor().spawner();
    if let Ok(t) = fetches() {
        spawner.spawn(t);
    }
    if let Ok(t) = models() {
        spawner.spawn(t);
    }
    if let Ok(t) = builds() {
        spawner.spawn(t);
    }
}

/// A turn that polled no task still asks for the next edge for the tasks it
/// queued.
pub fn ask_if_pended() {
    if *PENDED.get(|| false) {
        world_edge().ask_next();
    }
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
