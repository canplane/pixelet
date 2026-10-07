// pxl-kernel/door/executor.rs

//! embassy's raw executor behind xpute's `Executor`, installed at boot. The
//! pender only records that work is queued: poll is never called from it,
//! which the executor forbids, and `pending` tells the falling edge.

use crate::world::change::TASKS;
use embassy_executor::raw::Executor as Raw;
use xpute_guest::global::Global;
use xpute_guest::sched::task::{set_executor, Executor, Priority, Task};

struct OneThread;

critical_section::set_impl!(OneThread);

// SAFETY: the module runs on one thread and a turn never re-enters another.
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

#[embassy_executor::task(pool_size = TASKS)]
async fn run(task: Task) {
    task.await
}

struct Embassy;

impl Executor for Embassy {
    /// The priority is dropped: nothing relies on embassy's queue order, and
    /// its priority scheduler is left off.
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
        // SAFETY: called from the doorbell alone, which no turn re-enters, and
        // never from the pender.
        unsafe { raw().poll() };
    }

    fn pending(&self) -> bool {
        *PENDED.get(|| false)
    }
}

static EMBASSY: Embassy = Embassy;

/// Before the kernel spawns anything.
pub fn install() {
    set_executor(&EMBASSY);
}
