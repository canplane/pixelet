// pxl-gpu-host/vsync.rs

//! The display's frames for a guest on a native device, as vsync.ts is a
//! page's: `vsync.next` waits on the guest's thread until the platform's
//! display link calls `vsync`, and answers the frame's start on the host's
//! monotonic clock (xpute-host clock.rs `mark`).

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use xpute_host::clock::mark;
use xpute_host::runtime::runtime;
use xpute_kit::status::errno::Errno;
use xpute_link::Return;

use super::abi;

thread_local! {
    /// The guest's waiting `next`s by the count that keeps them apart, on the
    /// guest's thread.
    static WAITING: RefCell<BTreeMap<u64, Return<u64>>> = const { RefCell::new(BTreeMap::new()) };
    /// The next waiting `next`'s count.
    static NEXT: Cell<u64> = const { Cell::new(0) };
}

/// Whether the guest waits on a `next`: the display's frame is posted to the
/// guest's thread only then, as a page asks for an animation frame only for a
/// `next`.
static ASKED: AtomicBool = AtomicBool::new(false);

pub(super) struct Vsync;

impl abi::vsync::Host for Vsync {
    /// A wait its caller cancels leaves the waiting and answers ECANCELED,
    /// as the clock's does.
    fn next(&self, task_return: Return<u64>) {
        let id = NEXT.replace(NEXT.get() + 1);
        task_return.on_cancel(move || {
            if let Some(r) = WAITING.with_borrow_mut(|w| w.remove(&id)) {
                r.answer(Err(Errno::ECANCELED));
            }
        });
        WAITING.with_borrow_mut(|w| w.insert(id, task_return));
        ASKED.store(true, Ordering::Release);
    }
}

/// The display's frame, which started `age_ns` nanoseconds ago on the
/// platform's clock, from the platform's display link on any thread: the
/// guest's waiting `next` is answered with the frame's start, on its thread.
pub fn vsync(age_ns: u64) {
    if !ASKED.swap(false, Ordering::AcqRel) {
        return;
    }
    let at = mark(Instant::now()).saturating_sub(age_ns);
    (runtime().post)(Box::new(move || {
        for task_return in WAITING.with_borrow_mut(core::mem::take).into_values() {
            task_return.answer(Ok(at));
        }
    }));
}
