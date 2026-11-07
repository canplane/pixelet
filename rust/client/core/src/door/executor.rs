// pxl-core/door/executor.rs

//! embassy's raw executor behind xpute's `Executor`, installed at boot. The
//! pender only records that work is queued: poll is never called from it,
//! which the executor forbids, and `pending` tells the falling edge. No
//! core task is ordered before another, so each is spawned at priority 0
//! and the priority is dropped.

use crate::domain::scene::model::asset::REGISTRATIONS;
use crate::sys::load::LOAD_FILES;
use embassy_executor::raw::Executor as Raw;
use xpute_core::global::Global;
use xpute_core::sched::task::{self, Executor, Priority, Task};

/// The core's tasks, the executor's pool: the two that run for good
/// (turn.rs `world_tasks`), one for each file in flight, a slot's, the
/// catalog's or the sphere's (sys/io/loader.rs `arrival`), and one for each
/// model registering (domain/scene/model/asset.rs `registration`).
pub(crate) const TASKS: usize = 2 + LOAD_FILES + REGISTRATIONS;

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
    task.await;
}

struct Embassy;

impl Executor for Embassy {
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

/// Before the core spawns anything.
pub(super) fn install() {
    task::set(&EMBASSY);
}
