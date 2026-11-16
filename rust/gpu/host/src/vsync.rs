// pxl-gpu-host/vsync.rs

//! The display's frames for a guest on a native device, as vsync.ts is a
//! page's: `vsync.next` waits on the guest's thread until the platform's
//! display link calls `vsync` with a frame that starts at its `after` or
//! later, and answers that frame's start on the host's monotonic clock
//! (xpute-host clock.rs `mark`) and its gap since the frame before. The
//! display link calls `vsync` at every frame while it runs, so each gap is one
//! frame's but across a pause, as while the app is in the background.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use xpute_host::clock::mark;
use xpute_host::runtime::runtime;
use xpute_kit::status::errno::Errno;
use xpute_link::Return;

use super::abi;
use super::abi::vsync::Frame;

thread_local! {
    /// The guest's waiting `next`s by the count that keeps them apart, each
    /// with its `after`, on the guest's thread.
    static WAITING: RefCell<BTreeMap<u64, (u64, Return<Frame>)>> = const { RefCell::new(BTreeMap::new()) };
    /// The next waiting `next`'s count.
    static NEXT: Cell<u64> = const { Cell::new(0) };
}

/// The earliest `after` the guest waits on, `u64::MAX` for none: a frame is
/// posted to the guest's thread only from then, so a frame no wait reaches
/// costs the guest's thread nothing.
static EARLIEST: AtomicU64 = AtomicU64::new(u64::MAX);

/// The last frame's start, `u64::MAX` before the first, from which a gap
/// saturates to none.
static LAST: AtomicU64 = AtomicU64::new(u64::MAX);

pub(super) struct Vsync;

impl abi::vsync::Host for Vsync {
    /// A wait its caller cancels leaves the waiting and answers ECANCELED,
    /// as the clock's does.
    fn next(&self, after: u64, task_return: Return<Frame>) {
        let id = NEXT.replace(NEXT.get() + 1);
        task_return.on_cancel(move || {
            if let Some((_, r)) = WAITING.with_borrow_mut(|w| w.remove(&id)) {
                r.answer(Err(Errno::ECANCELED));
            }
        });
        WAITING.with_borrow_mut(|w| w.insert(id, (after, task_return)));
        EARLIEST.fetch_min(after, Ordering::AcqRel);
    }
}

/// The display's frame, which started `age_ns` nanoseconds ago on the
/// platform's clock, from the platform's display link on any thread: each
/// waiting `next` whose `after` it reaches is answered with it, on the
/// guest's thread.
pub fn vsync(age_ns: u64) {
    let at = mark(Instant::now()).saturating_sub(age_ns);
    let gap = at.saturating_sub(LAST.swap(at, Ordering::AcqRel));
    if at < EARLIEST.load(Ordering::Acquire) {
        return;
    }
    EARLIEST.store(u64::MAX, Ordering::Release);
    (runtime().post)(Box::new(move || {
        let reached = WAITING.with_borrow_mut(|w| {
            let (reached, left) = core::mem::take(w)
                .into_iter()
                .partition::<BTreeMap<_, _>, _>(|(_, (after, _))| *after <= at);
            if let Some(earliest) = left.values().map(|(after, _)| *after).min() {
                EARLIEST.fetch_min(earliest, Ordering::AcqRel);
            }
            *w = left;
            reached
        });
        for (_, task_return) in reached.into_values() {
            task_return.answer(Ok(Frame { at, gap }));
        }
    }));
}

#[cfg(test)]
#[path = "vsync.test.rs"]
mod test;
