// xpute-gpu-native-host/vsync.rs

//! The display's frames for a guest on a native device, as vsync.ts is a
//! page's: `vsync.next` waits on the guest's thread until the platform's
//! display link calls `vsync`.

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};

use xpute_link::Return;
use xpute_native_runtime::post_task;

use xpute_gpu_native_runtime::abi;

thread_local! {
    /// The guest's waiting `next`, on the guest's thread.
    static WAITING: RefCell<Vec<Return<()>>> = const { RefCell::new(Vec::new()) };
}

/// Whether the guest waits on a `next`: the display's frame is posted to the
/// guest's thread only then, as a page asks for an animation frame only for a
/// `next`.
static ASKED: AtomicBool = AtomicBool::new(false);

pub(super) struct Vsync;

impl abi::vsync::Host for Vsync {
    fn next(&self, task_return: Return<()>) {
        WAITING.with_borrow_mut(|n| n.push(task_return));
        ASKED.store(true, Ordering::Release);
    }
}

/// The display's frame, from the platform's display link on any thread: the
/// guest's waiting `next` is answered, on its thread.
pub fn vsync() {
    if !ASKED.swap(false, Ordering::AcqRel) {
        return;
    }
    post_task(Box::new(|| {
        for task_return in WAITING.with_borrow_mut(core::mem::take) {
            task_return.answer(Ok(()));
        }
    }));
}
