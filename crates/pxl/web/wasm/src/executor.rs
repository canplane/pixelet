// pxl-wasm/executor.rs

//! The executor the module installs for the kernel's tasks, as it installs the
//! slab for its memory: embassy's raw executor behind xpute_runtime::sched::task's
//! `Executor`. pxl-kernel spawns and polls through that interface and never
//! names this; which tasks there are is pxl-kernel's.
//!
//! A task reaches here type-erased and is run by the one task type below,
//! pooled as many times as the kernel declares it spawns (TASKS).
//!
//! The executor tells its owner it has work through the pender, a function it
//! links to by name. Here that only records the fact: poll is never called
//! from it, which the executor forbids, and `pending` is how the kernel's
//! falling edge learns a task still waits.
//!
//! One thread, and no turn re-enters another, so the critical section the
//! executor takes has nothing to exclude.

use embassy_executor::raw::Executor as Raw;
use pxl_kernel::global::Global;
use pxl_kernel::world::change::TASKS;
use xpute_runtime::sched::task::{set_executor, Executor, Priority, Task};

struct OneThread;

critical_section::set_impl!(OneThread);

// SAFETY: the module runs on one thread and a turn never re-enters another.
unsafe impl critical_section::Impl for OneThread {
    unsafe fn acquire() {}

    unsafe fn release(_: ()) {}
}

static RAW: Global<Raw> = Global::new();

/// Set by the pender: a task was queued since the last poll began.
static PENDED: Global<bool> = Global::new();

#[unsafe(export_name = "__pender")]
fn pender(_: *mut ()) {
    PENDED.set(true);
}

fn raw() -> &'static Raw {
    RAW.get(|| Raw::new(core::ptr::null_mut()))
}

#[embassy_executor::task(pool_size = TASKS)]
async fn run(task: Task) {
    task.await
}

struct Embassy;

impl Executor for Embassy {
    /// The priority is not kept: embassy's own queue has no order the kernel
    /// relies on, and its priority scheduler is a feature this build leaves off.
    fn spawn(&self, task: Task, _: Priority) -> bool {
        match run(task) {
            Ok(token) => {
                raw().spawner().spawn(token);
                true
            }
            Err(_) => false,
        }
    }

    fn poll(&self) {
        PENDED.set(false);
        // SAFETY: called from the doorbell alone, which no turn re-enters,
        // and never from the pender.
        unsafe { raw().poll() };
    }

    fn pending(&self) -> bool {
        *PENDED.get(|| false)
    }
}

static EMBASSY: Embassy = Embassy;

/// Installs the executor, before the kernel spawns anything.
pub fn install() {
    set_executor(&EMBASSY);
}
